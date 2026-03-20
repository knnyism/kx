use hassle_rs::Dxc;

fn main() {
    let dxc = Dxc::new(None).expect("failed to load DXC");

    let compiler = dxc.create_compiler().unwrap();
    let library = dxc.create_library().unwrap();
}
