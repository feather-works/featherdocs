//! `.docx` → modelo → `.docx`: `cargo run --example ida_e_volta_docx -- entrada.docx saida.docx`.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bytes = std::fs::read(&a[1]).expect("ler");
    let doc = featherdocs_io::docx::ler(&bytes).expect("abrir");
    for aviso in featherdocs_io::docx::avisos(&bytes, &doc) {
        eprintln!("aviso: {aviso}");
    }
    std::fs::write(&a[2], featherdocs_io::docx::gravar(&doc).expect("gravar")).expect("escrever");
}
