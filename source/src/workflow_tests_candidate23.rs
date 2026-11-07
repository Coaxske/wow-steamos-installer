#[test]
fn c23_proton_completion_and_refusals() {
    let f=Fixture::new();
    let req=json!({"steam":f.ctx.home.join(".local/share/Steam"),"proton":"proton_11"});
    let steam=f.ctx.home.join(".local/share/Steam");
    let acf=steam.join("steamapps/appmanifest_4628710.acf");
    let original=fs::read(&acf).unwrap();
    assert_eq!(proton_status(&f.ctx,&req).unwrap()["ready"],true);
    // Existing usable installation is reused; no external Steam process is started.
    assert_eq!(proton_install(&f.ctx,&req).unwrap()["ready"],true);
    fs::remove_file(&acf).unwrap();
    assert_eq!(proton_status(&f.ctx,&req).unwrap()["ready"],false);
    let setup=f.setup_request();
    assert!(review_setup(&f.ctx,&setup).is_err());
    for body in [
        "\"AppState\" {\"appid\" \"4628710\" \"installdir\" \"Proton 11\" \"SizeOnDisk\" \"1\" \"StateFlags\" \"1026\"}",
        "\"AppState\" {\"appid\" \"4628710\" \"installdir\" \"Proton 11\" \"SizeOnDisk\" \"1\" \"StateFlags\" \"4\" \"BytesToDownload\" \"20\" \"BytesDownloaded\" \"10\"}",
        "\"AppState\" {\"appid\" \"4628710\" \"installdir\" \"../escape\" \"SizeOnDisk\" \"1\" \"StateFlags\" \"4\"}",
        "malformed",
    ] {
        fs::write(&acf,body).unwrap();assert_eq!(proton_status(&f.ctx,&req).unwrap()["ready"],false);
    }
    fs::write(&acf,original).unwrap();
    let wine=steam.join("steamapps/common/Proton 11/files/bin/wine");
    fs::remove_file(&wine).unwrap();assert_eq!(proton_status(&f.ctx,&req).unwrap()["ready"],false);
    fs::write(&wine,b"fixture").unwrap();
    let tool=steam.join("steamapps/common/Proton 11/toolmanifest.vdf");
    fs::write(tool,b"\"manifest\" {\"commandline\" \"/proton %verb%\" \"require_tool_appid\" \"4183110\"}").unwrap();
    assert_eq!(proton_status(&f.ctx,&req).unwrap()["ready"],false);
    let runtime=steam.join("steamapps/common/SteamLinuxRuntime_4");fs::create_dir_all(&runtime).unwrap();
    fs::write(steam.join("steamapps/appmanifest_4183110.acf"),b"\"AppState\" {\"appid\" \"4183110\" \"installdir\" \"SteamLinuxRuntime_4\" \"SizeOnDisk\" \"1\" \"StateFlags\" \"4\"}").unwrap();
    fs::write(runtime.join("toolmanifest.vdf"),b"fixture").unwrap();fs::write(runtime.join("_v2-entry-point"),b"fixture").unwrap();
    assert_eq!(proton_status(&f.ctx,&req).unwrap()["ready"],true);
    assert!(review_setup(&f.ctx,&setup).is_ok());
    let bad=json!({"steam":f.ctx.home.join("elsewhere")});assert!(proton_status(&f.ctx,&bad).is_err());
}
#[test]
fn c23_proton_request_completion_errors() {
    let mut command=Command::new("/bin/sh");command.args(["-c","exit 1"]);
    assert!(send_proton_request(command).unwrap_err().contains("rejected"));
    let mut command=Command::new("/bin/sh");command.args(["-c","sleep 0.05; exit 0"]);
    assert!(send_proton_request(command).is_ok());
    let command=Command::new("/nonexistent/c23-fixture-command");
    assert!(send_proton_request(command).unwrap_err().contains("could not be opened"));
}

#[test]
fn c36_completed_proton_missing_runtime_has_explicit_separate_install_target() {
    let f = Fixture::new();
    let steam = f.ctx.home.join(".local/share/Steam");
    let tool = steam.join("steamapps/common/Proton 11/toolmanifest.vdf");
    fs::write(tool, b"\"manifest\" {\"commandline\" \"/proton %verb%\" \"require_tool_appid\" \"4183110\"}").unwrap();
    let request = json!({"steam":steam,"proton":"proton_11"});
    let status = proton_status(&f.ctx, &request).unwrap();
    assert_eq!(status["ready"], false);
    assert_eq!(status["missing_runtime"], "4183110");
    assert!(status["message"].as_str().unwrap().contains("Proton 11 is installed"));
    assert!(proton_install_target(&status, false).is_err());
    assert_eq!(proton_install_target(&status, true).unwrap(), "4183110");
    assert!(proton_install_target(&json!({"missing_runtime":"999999"}), true).is_err());
    assert!(proton_install_target(&json!({"ready":false}), true).is_err());
}
