use pulse::{
    model::{Scope, Service},
    parsing::parse_services,
    systemd::{Discovery, ScopeDiscovery},
};

pub fn services(scope: Scope) -> Vec<Service> {
    parse_services(include_bytes!("../fixtures/services.txt"), scope).unwrap()
}

pub fn report(scope: Scope, services: Vec<Service>) -> ScopeDiscovery {
    ScopeDiscovery {
        scope,
        result: Ok(Discovery {
            services,
            warning: String::new(),
        }),
    }
}
