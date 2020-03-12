pub mod service {
    tonic::include_proto!("com.github.lavignes.roolz.api.v1alpha.service");

    // re-export client/server
    pub use self::rules_service_client::*;
    pub use self::rules_service_server::*;
}