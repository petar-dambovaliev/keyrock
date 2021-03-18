fn main() {
    tonic_build::configure()
        .compile(&["proto/order_book.proto"], &["proto"])
        .unwrap()
}
