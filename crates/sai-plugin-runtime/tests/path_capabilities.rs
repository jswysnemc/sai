use sai_plugin_runtime::{BinaryCapabilities, SystemCapabilities};

/// 【路径授权测试】【短文件名】目录和文件名中的波浪号按字面值校验，兼容 Windows 短路径。
/// @returns 无；读取声明、写入声明及请求前置校验均接受合法路径
#[test]
fn literal_tildes_in_path_components_are_valid_for_reading_and_writing() {
    for path in [
        "a~",
        r"C:\Users\RUNNER_1\AppData\Local\Temp\pictures\web-images",
        r"C:\Users\RUNNER~1\AppData\Local\Temp\pictures\web-images",
        "C:/Users/RUNNER~1/AppData/Local/Temp/pictures/web-images",
        "RUNNER~1/pictures/web-images",
        "/tmp/RUNNER~1/pictures/web-images",
        "cache/backup~",
        "cache/~literal/image.png",
        "~",
        "~/pictures/backup~",
    ] {
        let read = SystemCapabilities {
            read_paths: [path.into()].into(),
            ..Default::default()
        };
        let write = BinaryCapabilities {
            write_paths: [path.into()].into(),
            ..Default::default()
        };
        read.validate()
            .unwrap_or_else(|error| panic!("read declaration {path}: {error}"));
        assert!(write.validate().is_ok(), "write declaration: {path}");
        assert!(
            read.check_read_request(path).is_ok(),
            "read request: {path}"
        );
        assert!(
            write.check_write(path, true).is_ok(),
            "write request: {path}"
        );
        assert!(write.check_write(path, false).is_err(), "read-only: {path}");
    }
}

/// 【路径授权测试】【语法边界】允许字面波浪号不等于支持其他用户目录、通配符或父目录跳转。
/// @returns 无；非法声明与读写请求均在宿主 I/O 前失败
#[test]
fn unsupported_home_shortcuts_and_unsafe_paths_remain_rejected() {
    for path in [
        "",
        "~other",
        "~other/pictures",
        r"~other\pictures",
        r"~\pictures",
        "~+",
        "~-",
        "cache~1/../outside",
        r"C:\Users\RUNNER~1\..\outside",
        "cache~1/*.png",
        "cache~1/image?.png",
        "cache~1/<image>",
        "cache~1/file|name",
        "cache~1/\"image\"",
        "cache~1/image\0.png",
        "cache~1/image\n.png",
    ] {
        let read = SystemCapabilities {
            read_paths: [path.into()].into(),
            ..Default::default()
        };
        let write = BinaryCapabilities {
            write_paths: [path.into()].into(),
            ..Default::default()
        };
        assert!(read.validate().is_err(), "read declaration: {path:?}");
        assert!(write.validate().is_err(), "write declaration: {path:?}");
        assert!(
            read.check_read_request(path).is_err(),
            "read request: {path:?}"
        );
        assert!(
            write.check_write(path, true).is_err(),
            "write request: {path:?}"
        );
    }
}

/// 【路径授权测试】【精确目录】字面波浪号不参与路径展开，声明变化仍然撤销原有授权。
/// @returns 无；不同路径的交集为空，空授权继续拒绝读取和写入
#[test]
fn literal_tildes_do_not_expand_or_create_path_grants() {
    let read = SystemCapabilities {
        read_paths: ["RUNNER~1/cache".into()].into(),
        ..Default::default()
    };
    let other_read = SystemCapabilities {
        read_paths: ["RUNNER~2/cache".into()].into(),
        ..Default::default()
    };
    let write = BinaryCapabilities {
        write_paths: read.read_paths.clone(),
        ..Default::default()
    };
    let other_write = BinaryCapabilities {
        write_paths: other_read.read_paths.clone(),
        ..Default::default()
    };
    read.validate().unwrap();
    write.validate().unwrap();
    assert!(read.intersection(&other_read).read_paths.is_empty());
    assert!(write.intersection(&other_write).write_paths.is_empty());
    assert!(!read.is_subset(&other_read));
    assert!(!write.is_subset(&other_write));
    assert!(SystemCapabilities::default()
        .check_read_request("RUNNER~1/cache/file")
        .is_err());
    assert!(BinaryCapabilities::default()
        .check_write("RUNNER~1/cache/file", true)
        .is_err());
}
