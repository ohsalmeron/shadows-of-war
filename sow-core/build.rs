fn main() {
    println!("cargo:rerun-if-changed=../assets/static/maps");
    let path = "../assets/static/maps/northamerica/map.bin.br";
    println!("cargo:rerun-if-changed={path}");
}
