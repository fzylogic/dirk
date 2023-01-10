/*    use std::process::Command;
pub fn docker_examine() {
    let docker = Command::new("docker")
        .arg("--rm")
        .arg("--network none")
        .arg("-u sandbox")
        .arg("-v -v ~/code:/usr/src/sandbox")
        .arg("-v -v ~/results:/usr/src/results")
        .arg("-w /usr/src/sandbox")
        .arg("dreamhost/php-8.0-xdebug:production")
        .arg("bash /usr/local/bin/check.sh");
}*/
/* Workflow is as follows:
 * Client uploads files via the API
 * Server then dumps the files into a tempdir
 * A container is spun up using our custom php/xdebug image w/ the tempdir mounted as a read-only volume
 * This container has no networking or other privileges
 * A second read-only volume is mounted, which contains a socket for communication back to the host.+
 * Once analysis is complete and the results have been reported back via the socket, the container is shut down
 */

use std::collections::HashSet;
use std::fs::File;
use std::io::prelude::*;

use base64::{engine::general_purpose, Engine as _};
use podman_api::models::ContainerMount;
use podman_api::opts::ContainerCreateOpts;
use podman_api::Podman;
use tempfile::TempDir;
use tokio::time;

use crate::errors::DynamicScanError::ContainerCreationError;
use crate::errors::*;
use crate::models::dirk::{ScanBulkRequest, ScanRequest};
use crate::phpxdebug;
use crate::phpxdebug::Tests;

// This is meant to eventually dump an entire collection
// of files into a temp dir in order to scan themm all as
// a single unit.
#[allow(dead_code)]
fn prep_dir(dir: TempDir, requests: ScanBulkRequest) -> Result<(), DirkError> {
    for req in requests.requests {
        let prefix_path = dir.path().join(req.file_name.parent().unwrap());
        let builder = std::fs::DirBuilder::new()
            .recursive(true)
            .create(&prefix_path);
        match builder {
            Ok(_) => {
                let mut file = File::create(req.file_name.file_name().unwrap())?;
                file.write_all(req.file_contents.unwrap_or_default().as_bytes())?;
            }
            Err(e) => eprintln!(
                "Encountered error while attempting ot create dir `{}`: {e}",
                prefix_path.display()
            ),
        }
    }
    Ok(())
}

// TODO Change this return type to a custom Result
/// Runs a dynamic scan on a single file via a ScanRequest
pub async fn examine_one(
    dir: TempDir,
    request: &ScanRequest,
) -> Result<HashSet<Tests>, DynamicScanError> {
    let podman = Podman::unix("/run/user/1000/podman/podman.sock");
    let tmpfile = dir.path().join("testme.php");
    let mut file = File::create(&tmpfile).unwrap();
    file.write_all(
        &general_purpose::STANDARD
            .decode(request.file_contents.as_ref().unwrap())
            .unwrap(),
    )?;
    println!("Wrote data to {}", &tmpfile.display());
    let mount = ContainerMount {
        destination: Some("/usr/local/src".to_string()),
        options: None,
        source: Some(dir.path().to_string_lossy().parse().unwrap()),
        _type: Some("bind".to_string()),
        uid_mappings: None,
        gid_mappings: None,
    };
    let container = podman
        .containers()
        .create(
            &ContainerCreateOpts::builder()
                .image("dreamhost/php-8.0-xdebug:production")
                .command([
                    "/usr/local/bin/php",
                    "-d",
                    "xdebug.output_dir=/usr/local/src",
                    "-d",
                    "xdebug.trace_output_name=outfile",
                    "/usr/local/src/testme.php",
                ])
                .remove(true)
                .mounts(vec![mount])
                .no_new_privilages(true)
                .timeout(60u64)
                .build(),
        )
        .await;
    match container {
        Ok(id) => {
            let _start_result = podman.containers().get(id.id).start(None).await;
            let outfile = dir.path().join("outfile.xt");
            let mut try_counter: u8 = 0;
            loop {
                if outfile.exists() {
                    break;
                } else if try_counter >= 60 {
                    eprintln!("Gave up waiting for output file to exist");
                    return Err(DynamicScanError::ResultNotFound);
                }
                try_counter += 1;
                time::sleep(time::Duration::from_millis(500)).await;
            }
            let record = phpxdebug_parser::parse_xtrace_file(outfile.as_path());
            match record {
                Ok(record) => {
                    let results = phpxdebug::analyze(&record);
                    println!("{:#?}", results);
                    Ok(results)
                }
                Err(e) => {
                    eprintln!("{e}");
                    time::sleep(time::Duration::from_secs(300)).await;
                    Err(DynamicScanError::ResultNotFound)
                }
            }
        }
        Err(e) => {
            eprintln!("{e}");
            Err(ContainerCreationError)
        }
    }
}
