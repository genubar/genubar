use crate::printer::print_tree;
use genubar_core::{MenuProvider, MenuTarget};
use genubar_dbus::unified::UnifiedMenuService;
use std::error::Error;

pub async fn discover(service: &UnifiedMenuService, dest: String) -> Result<(), Box<dyn Error>> {
    let paths = service.discover_paths(&dest).await?;
    for (path, iface) in paths {
        println!("  {path:<45} -> {iface}");
    }
    Ok(())
}

pub async fn inspect_canonical(
    service: &UnifiedMenuService,
    dest: String,
    path: String,
) -> Result<(), Box<dyn Error>> {
    let target = MenuTarget::Canonical {
        bus_name: dest,
        object_path: path,
    };
    let root = service.fetch_menu(&target).await?;
    print_tree(&root, 0);
    Ok(())
}

pub async fn inspect_gtk(
    service: &UnifiedMenuService,
    dest: String,
    menubar_path: String,
    app_menu_path: Option<String>,
    app_path: Option<String>,
    win_path: Option<String>,
) -> Result<(), Box<dyn Error>> {
    let target = MenuTarget::Gtk {
        bus_name: dest,
        menubar_path: Some(menubar_path),
        app_menu_path,
        app_path,
        win_path,
    };
    let root = service.fetch_menu(&target).await?;
    print_tree(&root, 0);
    Ok(())
}

pub async fn inspect_atspi(service: &UnifiedMenuService, pid: u32) -> Result<(), Box<dyn Error>> {
    if let Some(target) = service.atspi().discover_target_by_pid(pid).await {
        println!("Discovered AT-SPI2 MenuBar target: {target:?}");
        let root = service.fetch_menu(&target).await?;
        print_tree(&root, 0);
    } else {
        println!("No AT-SPI2 'menu bar' found for PID {pid}");
    }
    Ok(())
}

pub async fn dump_atspi(
    service: &UnifiedMenuService,
    pid: u32,
    depth: usize,
    include_doc: bool,
) -> Result<(), Box<dyn Error>> {
    let report = service
        .atspi()
        .dump_app_tree(pid, depth, include_doc)
        .await?;
    println!("{report}");
    Ok(())
}

pub async fn click_canonical(
    service: &UnifiedMenuService,
    dest: String,
    path: String,
    id: i32,
) -> Result<(), Box<dyn Error>> {
    let target = MenuTarget::Canonical {
        bus_name: dest,
        object_path: path,
    };
    service.activate_item(&target, id).await?;
    println!("Triggered Canonical click on ID {id}");
    Ok(())
}

pub async fn click_gtk(
    service: &UnifiedMenuService,
    dest: String,
    menubar_path: String,
    app_path: Option<String>,
    win_path: Option<String>,
    id: i32,
) -> Result<(), Box<dyn Error>> {
    let target = MenuTarget::Gtk {
        bus_name: dest,
        menubar_path: Some(menubar_path),
        app_menu_path: None,
        app_path,
        win_path,
    };
    service.activate_item(&target, id).await?;
    println!("Triggered GTK Action click on ID {id}");
    Ok(())
}

pub async fn dump_gtk4(service: &UnifiedMenuService, pid: u32) -> Result<(), Box<dyn Error>> {
    if let Some(MenuTarget::Gtk { bus_name, .. }) = service.discover_dbus_target_by_pid(pid).await {
        let report = service.gtk().dump_gtk4_introspection(&bus_name).await?;
        println!("{report}");
    } else {
        println!(
            "No hooked GTK4 D-Bus target found for PID {pid}. Check ~/.cache/genubar/gtk4_hook.log"
        );
    }
    Ok(())
}
