.pragma library
function defaultProton(tools) {
    for (var n=0;n<tools.length;n++)
        if (/^proton_11(?:0)?$/.test(tools[n].name)) return tools[n].name;
    return "";
}
function step(installation, exitStatus, singleTileReady) {
    if (!installation || !installation.id) return "start";
    if (installation.stage === "launcher_install") return "launcher";
    if (installation.launcher && (!installation.launcher_exit_reviewed || !exitStatus || exitStatus.ready!==true)) return "launcher_behavior";
    if (installation.stage === "games") return "download";
    if (installation.restore_backup) return "restore";
    if (singleTileReady===false) return "shortcuts";
    if (installation.stage === "complete") return "complete";
    if (installation.stage === "addons") return "test";
    return "test";
}
function afterApply(kind) {
    if (kind === "addon") return "addons";
    if (kind === "clean" || kind === "clean_owned" || kind === "remove" || kind === "legacy") return "home";
    return "games";
}
function planText(kind) {
    var texts = {
        setup: "Install Battle.net and add it to Steam. Your existing games will stay in place.",
        launcher: "Update the Steam shortcut to open the installed Battle.net app.",
        repair: "Create or reuse exactly one Battle.net tile. Choose the edition inside Battle.net and press Play. Game files stay in place.",
        adopt: "Use this Battle.net installation and its Steam shortcuts. Keep the existing Proton choice and game files.",
        addon: "Add the addon manager to Steam so you can open it from your library.",
        exit: "Review this installation’s Battle.net behavior. Close after Play is required to finish this release setup and affects BOTH Desktop and Gaming Mode; Keep current leaves vendor preferences untouched and setup incomplete unless the setting is already enabled. Close Battle.net and WoW normally before continuing.",
        reinstall: "Remove the listed folders and set up a fresh copy. You will need to sign in and download your games again."
    };
    return texts[kind] || "Make the changes listed below.";
}

// Retain an explicit valid selection. Never replace a disappeared selection.
function selection(all, selected, saved, initialized, unavailable) {
    function valid(id) {return !!id && unavailable.indexOf(id)<0 && all.some(function(i){return i.id===id})}
    if (valid(selected)) return selected;
    if (selected || initialized) return "";
    return valid(saved) ? saved : "";
}
