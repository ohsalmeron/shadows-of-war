fn main() {
    println!("cargo:rerun-if-changed=../assets/maps");
    let path = "../assets/maps/northamerica/map.bin.br";
    println!("cargo:rerun-if-changed={path}");
}
