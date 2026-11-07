use serde::Serialize;
#[derive(Debug, Clone, Serialize)]
pub struct Edition {
    pub id: &'static str,
    pub name: &'static str,
    pub folders: &'static [&'static str],
    pub note: &'static str,
    pub command: Option<&'static str>,
}
pub const EDITIONS: &[Edition] = &[
    Edition {
        id: "ptr_next",
        name: "WoW Retail second PTR",
        folders: &["_xptr_"],
        note: "Select the intended test product in Battle.net.",
        command: None,
    },
    Edition {
        id: "retail",
        name: "World of Warcraft",
        folders: &["_retail_"],
        note: "Retail client",
        command: Some("WoW"),
    },
    Edition {
        id: "forever",
        name: "World of Warcraft Forever",
        folders: &["_forever_"],
        note: "Confirm the product in Battle.net; do not assume a beta folder is Forever.",
        command: Some("WoWF"),
    },
    Edition {
        id: "era",
        name: "WoW Classic Era",
        folders: &["_classic_era_"],
        note: "Hardcore and Season of Discovery are selected in game; one shared client.",
        command: Some("WoWC"),
    },
    Edition {
        id: "progression",
        name: "WoW Classic Progression",
        folders: &["_classic_"],
        note: "The expansion name changes. Select the product in Battle.net.",
        command: None,
    },
    Edition {
        id: "anniversary",
        name: "WoW Classic Anniversary",
        folders: &["_anniversary_", "_classic_anniversary_"],
        note: "May share another client. Do not create a second tile for the same folder.",
        command: None,
    },
    Edition {
        id: "ptr",
        name: "WoW Retail PTR",
        folders: &["_ptr_"],
        note: "Availability depends on Blizzard.",
        command: None,
    },
    Edition {
        id: "beta",
        name: "WoW Retail Beta",
        folders: &["_beta_"],
        note: "Account access required.",
        command: None,
    },
    Edition {
        id: "classic_ptr",
        name: "WoW Classic PTR",
        folders: &["_classic_ptr_"],
        note: "Select the product in Battle.net.",
        command: None,
    },
    Edition {
        id: "classic_beta",
        name: "WoW Classic / Forever Beta",
        folders: &["_classic_beta_"],
        note: "Reused folder: confirm the product before setting a launch command.",
        command: None,
    },
    Edition {
        id: "era_ptr",
        name: "WoW Classic Era PTR",
        folders: &["_classic_era_ptr_"],
        note: "Availability depends on Blizzard.",
        command: None,
    },
    Edition {
        id: "alpha",
        name: "WoW Retail Alpha",
        folders: &["_alpha_"],
        note: "Account access required.",
        command: None,
    },
    Edition {
        id: "titan",
        name: "WoW Classic Titan Reforged",
        folders: &["_classic_titan_"],
        note: "Region and account availability apply.",
        command: None,
    },
];
pub fn by_id(id: &str) -> Option<&'static Edition> {
    EDITIONS.iter().find(|x| x.id == id)
}
pub fn by_folder(name: &str) -> Option<&'static Edition> {
    EDITIONS.iter().find(|x| x.folders.contains(&name))
}
pub fn is_game_exe(name: &str) -> bool {
    [
        "wow.exe",
        "wow-64.exe",
        "wowclassic.exe",
        "wowclassict.exe",
        "wowclassicb.exe",
        "wowb.exe",
        "wowt.exe",
    ]
    .contains(&name.to_ascii_lowercase().as_str())
}
