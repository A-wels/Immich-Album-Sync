mod config;

use config::Config;
use std::path::PathBuf;
use std::fs;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashSet;
use chrono::Utc;
use std::process::Command; // Added for setup_scheduled_task
use std::io::{self, Read, Write}; // Added Write for flush, Read for pause

#[derive(Debug, Deserialize)]
struct Album {
    #[serde(rename = "id")]
    id: String,
    #[serde(rename = "albumName")]
    album_name: String,
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    #[serde(rename = "id")]
    id: String,
    #[serde(rename = "originalPath")]
    original_path: String,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // If started with --background, run sync directly (for scheduled task)
    if args.len() > 1 && args[1] == "--background" {
        run_sync();
        return;
    }    // Show a simple menu for setup or sync
    println!("Immich Album Sync");
    println!("=================");
    println!("1. Einmalig als Windows-Task einrichten (beim PC-Start ausführen)");
    println!("2. Jetzt synchronisieren");
    println!("3. Beenden");
    println!("");
    print!("Bitte wählen (1-3): ");
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    match input.trim() {
        "1" => {
            setup_scheduled_task();
        },
        "2" => {
            run_sync();
        },
        _ => {
            println!("Beendet.");
        }
    }
}


fn run_sync() {
    use std::fs::OpenOptions;
    use std::io::Write;
    
    // Get the directory where the executable is located (not the current working directory)
    let exe_path = std::env::current_exe().expect("Failed to get executable path");
    let exe_dir = exe_path.parent().expect("Failed to get executable directory");
    let config_path = exe_dir.join("config.json");

    // Find user's Documents folder
    let documents_dir = dirs::document_dir().unwrap_or_else(|| exe_dir.to_path_buf());    let log_path = documents_dir.join("ImmichAlbumSync.log");
    let mut log_file = OpenOptions::new().create(true).append(true).open(&log_path).expect("Failed to open log file");

    // Log startup information for debugging
    let startup_msg = format!("[{}] Starting sync - Executable: {}, Config path: {}\n", Utc::now(), exe_path.display(), config_path.display());
    let _ = log_file.write_all(startup_msg.as_bytes());let config = match Config::load(&config_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            let error_msg = format!("[{}] CRITICAL: Failed to load config.json from '{}': {}. Exiting sync.\n", Utc::now(), config_path.display(), e);
            eprint!("{}", error_msg); // Also print to console if run manually
            let _ = log_file.write_all(error_msg.as_bytes());
            return;
        }
    };

    if let Err(e) = fs::create_dir_all(&config.local_folder) {
        let error_msg = format!("[{}] CRITICAL: Failed to create local folder '{}': {}. Exiting sync.\n", Utc::now(), config.local_folder, e);
        eprint!("{}", error_msg);
        let _ = log_file.write_all(error_msg.as_bytes());
        return;
    }

    let client = Client::new();
    let album = match get_album_with_assets(&client, &config) {
        Ok(alb) => alb,
        Err(e) => {
            let error_msg = format!("[{}] CRITICAL: Failed to fetch album data from Immich API: {}. Exiting sync.\nCheck API URL, key, and network connectivity.\n", Utc::now(), e);
            eprint!("{}", error_msg);
            let _ = log_file.write_all(error_msg.as_bytes());
            return;
        }
    };
    let start_msg = format!("[{}] Album '{}' - {} assets\nDownload path: {}\n", Utc::now(), album.album_name, album.assets.len(), config.local_folder);
    print!("{}", start_msg);
    let _ = log_file.write_all(start_msg.as_bytes());

    // Identify and remove local files no longer present in the Immich album.
    let album_asset_ids: HashSet<String> = album.assets.iter().map(|a| a.id.clone()).collect();
    let mut local_files_to_remove: Vec<PathBuf> = Vec::new();

    if let Ok(entries) = fs::read_dir(&config.local_folder) {
        for entry in entries.filter_map(Result::ok) {
            let local_path = entry.path();
            if local_path.is_file() {
                if let Some(file_stem) = local_path.file_stem().and_then(|s| s.to_str()) {
                    // Assuming local filenames are derived from asset IDs (e.g., <asset_id>.<ext>).
                    // Check if the asset ID from the local filename exists in the current album.
                    if !album_asset_ids.contains(file_stem) {
                        local_files_to_remove.push(local_path.clone());
                        let delete_msg = format!("Local file {} is not in the album. Marking for deletion.\n", local_path.display());
                        print!("{}", delete_msg);
                        let _ = log_file.write_all(delete_msg.as_bytes());
                    }
                }
            }
        }
    }

    for file_to_remove in &local_files_to_remove {
        match fs::remove_file(file_to_remove) {
            Ok(_) => {
                let success_msg = format!("Deleted orphaned local file: {}\n", file_to_remove.display());
                print!("{}", success_msg);
                let _ = log_file.write_all(success_msg.as_bytes());
            }
            Err(e) => {
                let error_msg = format!("Failed to delete orphaned local file {}: {}\n", file_to_remove.display(), e);
                eprint!("{}", error_msg);
                let _ = log_file.write_all(error_msg.as_bytes());
            }
        }
    }

    let mut new = 0;
    let mut skip = 0;
    let mut fail = 0;

    // Iterate through assets in the Immich album to download missing files.
    for asset in &album.assets {
        let ext = std::path::Path::new(&asset.original_path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e))
            .unwrap_or_else(|| ".jpg".to_string());
        let file_path = PathBuf::from(&config.local_folder).join(format!("{}{}", asset.id, ext));

        // If the file already exists locally, skip downloading it.
        if file_path.exists() {
            skip += 1;
            continue; // Proceed to the next asset.
        }

        match download_asset(&client, &config, &asset.id, &file_path) {
            Ok(_) => {
                new += 1;
                let msg = format!("Downloaded: {} -> {}\n", asset.id, file_path.display());
                print!("{}", msg);
                let _ = log_file.write_all(msg.as_bytes());
            }
            Err(e) => {
                let msg = format!("Download failed for {}: {}\n", asset.id, e);
                eprint!("{}", msg);
                let _ = log_file.write_all(msg.as_bytes());
                fail += 1;
            }
        }
    }
    let finish_msg = format!("Finished - new:{}  skipped:{}  failed:{}\n\n", new, skip, fail);
    println!("{}", finish_msg.trim());
    let _ = log_file.write_all(finish_msg.as_bytes());
}

fn setup_scheduled_task() {
    println!("Einrichten des geplanten Tasks...");

    // Check for administrator privileges
    match is_running_as_admin() {
        Ok(true) => println!("Programm wird mit Administratorrechten ausgeführt."),
        Ok(false) => {
            eprintln!("FEHLER: Für das Einrichten des geplanten Tasks sind Administratorrechte erforderlich.");
            eprintln!("Bitte führen Sie das Programm als Administrator aus (Rechtsklick -> Als Administrator ausführen).");
            wait_for_enter();
            return;
        }
        Err(e) => {
            eprintln!("Warnung: Konnte nicht eindeutig feststellen, ob das Programm mit Administratorrechten läuft: {}", e);
            eprintln!("Versuche trotzdem, den Task zu erstellen...");
            // Proceed, schtasks will fail if not admin anyway, but this allows for edge cases where check might fail
        }
    }

    let exe_path = match std::env::current_exe() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Fehler: Ausführbarer Pfad konnte nicht ermittelt werden: {}", e);
            wait_for_enter();
            return;
        }
    };

    let exe_path_str = match exe_path.to_str() {
        Some(s) => s,
        None => { // This is a standard Option::None pattern, the linter warning about variable name is likely a false positive.
            eprintln!("Fehler: Ausführbarer Pfad ist kein gültiger UTF-8 String.");
            wait_for_enter();
            return;
        }
    };

    let task_name = "ImmichAlbumSync";
    // Ensure the command for powershell's ArgumentList is properly quoted if paths contain spaces.
    // The entire ArgumentList is a single string here.
    let task_command = format!(
        "powershell -Command Start-Process -WindowStyle Hidden -FilePath '{}' -ArgumentList '--background'",
        exe_path_str
    );

    println!("Versuche, den Task zu erstellen mit Befehl: {}", task_command);    // Get current username to run task as current user instead of SYSTEM
    let current_user = match std::env::var("USERNAME") {
        Ok(user) => user,
        Err(_) => {
            eprintln!("Warnung: Konnte Benutzername nicht ermitteln. Verwende Standard-Benutzerkontext.");
            "".to_string()
        }
    };

    let mut schtasks_args = vec![
        "/Create",
        "/TN", task_name,
        "/TR", &task_command,
        "/SC", "ONSTART",
        "/RL", "HIGHEST",
        "/F", // Force creation, overwrites if exists
    ];

    // Only add /RU parameter if we have a username
    if !current_user.is_empty() {
        schtasks_args.extend(["/RU", &current_user]);
        println!("Task wird als Benutzer '{}' ausgeführt.", current_user);
    } else {
        println!("Task wird im Standard-Benutzerkontext ausgeführt.");
    }

    let output = Command::new("schtasks")
        .args(schtasks_args)
        .output();

    match output {
        Ok(out) => {
            if out.status.success() {
                println!("Geplanter Task '{}' erfolgreich eingerichtet.", task_name);
                println!("Er wird bei jedem Systemstart im Hintergrund ausgeführt.");
            } else {
                eprintln!("Fehler beim Einrichten des geplanten Tasks:");
                eprintln!("Status: {}", out.status);
                if !out.stdout.is_empty() {
                    eprintln!("Stdout: {}", String::from_utf8_lossy(&out.stdout));
                }
                if !out.stderr.is_empty() {
                    eprintln!("Stderr: {}", String::from_utf8_lossy(&out.stderr));
                }
                eprintln!("\nStellen Sie sicher, dass das Programm mit Administratorrechten ausgeführt wird, falls der Fehler weiterhin besteht.");
            }
        }
        Err(e) => {
            eprintln!("Fehler beim Ausführen von schtasks.exe: {}", e);
            eprintln!("Stellen Sie sicher, dass schtasks.exe im Systempfad vorhanden ist.");
        }
    }
    wait_for_enter();
}

fn wait_for_enter() {
    println!("\nDrücken Sie die Eingabetaste, um fortzufahren...");
    io::stdout().flush().unwrap(); // Ensure Write trait is in scope
    let _ = io::stdin().read(&mut [0u8]);
}

fn get_album_with_assets(client: &Client, config: &Config) -> Result<Album, reqwest::Error> {
    let url = format!("{}/albums/{}?withoutAssets=false", config.api_url.trim_end_matches('/'), config.album_id);
    client
        .get(&url)
        .header("x-api-key", &config.api_key)
        .send()?
        .error_for_status()?
        .json::<Album>()
}

fn download_asset(
    client: &Client,
    config: &Config,
    asset_id: &str,
    target: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("{}/assets/{}/original", config.api_url.trim_end_matches('/'), asset_id);
    let mut resp = client
        .get(&url)
        .header("x-api-key", &config.api_key)
        .send()?;
    let mut out = fs::File::create(target)?;
    std::io::copy(&mut resp, &mut out)?;
    Ok(())
}

fn is_running_as_admin() -> Result<bool, std::io::Error> {
    let output = Command::new("whoami")
        .arg("/groups")
        .output()?;

    if !output.status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("whoami /groups failed with status: {}", output.status)
        ));
    }

    let groups_output = String::from_utf8_lossy(&output.stdout);
    // S-1-5-32-544 is the well-known SID for the BUILTIN\Administrators group.
    // S-1-16-12288 is for High Mandatory Level.
    Ok(groups_output.contains("S-1-5-32-544") && groups_output.contains("Enabled group") || groups_output.contains("S-1-16-12288"))
}


