use x_bot_follower_remover::config::default_data_dir;

#[test]
fn upgrade_reuses_data_and_lock_without_moving_a_running_workers_directory() {
    let temp = tempfile::tempdir().unwrap();
    assert_eq!(default_data_dir(temp.path()), temp.path().join("remover"));
    let legacy = temp.path().join("forgive-me");
    std::fs::create_dir(&legacy).unwrap();
    std::fs::write(legacy.join("cleanup.sqlite"), b"preserved").unwrap();
    std::fs::create_dir(temp.path().join("remover")).unwrap();
    assert_eq!(default_data_dir(temp.path()), legacy);
    assert_eq!(
        std::fs::read(legacy.join("cleanup.sqlite")).unwrap(),
        b"preserved"
    );
}
