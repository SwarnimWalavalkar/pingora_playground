use async_trait::async_trait;
use pingora::prelude::*;
use std::sync::Arc;

pub struct LB(Arc<LoadBalancer<RoundRobin>>);

#[async_trait]
impl ProxyHttp for LB {
    type CTX = ();
    fn new_ctx(&self) -> () {
        ()
    }

    async fn upstream_peer(&self, _session: &mut Session, _ctx: &mut ()) -> Result<Box<HttpPeer>> {
        let upstream = match self.0.select(b"", 256) {
            Some(upstream) => upstream,
            None => {
                panic!("Failed to select upstream");
            }
        };

        println!("upstream: {upstream:?}");

        let peer = Box::new(HttpPeer::new(upstream, true, "one.one.one.one".to_string()));
        Ok(peer)
    }

    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut RequestHeader,
        _ctx: &mut Self::CTX,
    ) -> Result<()> {
        upstream_request
            .insert_header("Host", "one.one.one.one")
            .unwrap();
        Ok(())
    }
}

pub fn main() {
    let mut my_server = match Server::new(Some(Opt::default())) {
        Ok(server) => server,
        Err(e) => {
            panic!("Failed to create server: {e:?}");
        }
    };

    my_server.bootstrap();

    // 127.0.0.1:343" is just a bad server
    let mut upstreams =
        LoadBalancer::try_from_iter(["1.1.1.1:443", "1.0.0.1:443", "127.0.0.1:343"]).unwrap();

    let hc = TcpHealthCheck::new();
    upstreams.set_health_check(hc);
    upstreams.health_check_frequency = Some(std::time::Duration::from_secs(1));

    let background = background_service("health check", upstreams);

    let upstreams = background.task();

    let mut lb = http_proxy_service(&my_server.configuration, LB(upstreams));
    lb.add_tcp("0.0.0.0:6188");

    // let cert_path = format!("{}/tests/keys/server.crt", env!("CARGO_MANIFEST_DIR"));
    // let key_path = format!("{}/tests/keys/key.pem", env!("CARGO_MANIFEST_DIR"));

    // let mut tls_settings =
    //     pingora::listeners::TlsSettings::intermediate(&cert_path, &key_path).unwrap();
    // tls_settings.enable_h2();
    // lb.add_tls_with_settings("0.0.0.0:6189", None, tls_settings);

    my_server.add_service(background);

    my_server.add_service(lb);
    my_server.run_forever();
}
