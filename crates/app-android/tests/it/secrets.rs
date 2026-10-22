//! The credential store of the platform. Live: on a device, inside the app's process (keyring's
//! Android store needs the ndk-context that `Native.open` sets up), with
//! `DAIL_IT_ANDROID_KEYSTORE=1`.

use app_core::{Secret, SecretRef};

#[test]
#[ignore = "live: needs a device and the app's ndk-context; set DAIL_IT_ANDROID_KEYSTORE=1"]
fn a_password_round_trips_through_the_store() {
    if std::env::var_os("DAIL_IT_ANDROID_KEYSTORE").is_none() {
        return;
    }
    let store = dail::secrets::store().unwrap();
    let key = SecretRef {
        service: "dail-it".to_owned(),
        user: "round-trip@example.com".to_owned(),
    };
    store
        .set(&key, &Secret::new("not-a-real-password"))
        .unwrap();
    assert_eq!(
        store.get(&key).unwrap().map(|s| s.expose().to_owned()),
        Some("not-a-real-password".to_owned())
    );
    store.delete(&key).unwrap();
    assert!(store.get(&key).unwrap().is_none());
    store.delete(&key).unwrap();
}
