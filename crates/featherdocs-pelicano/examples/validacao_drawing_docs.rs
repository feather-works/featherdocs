//! Exemplo DRW-013 — vetores da pagina real do FeatherDocs -> Drawing -> SVG.
//! O SVG contém APENAS vetores; não é um exportador de documento completo.
//! cargo run --manifest-path featherdocs/crates/featherdocs-pelicano/Cargo.toml \
//!   --example validacao_drawing_docs -j 2 > /tmp/featherdocs-drawing.svg
use featherdocs_core::sessao::Sessao;
use featherdocs_pelicano::{desenho_compartilhado::cena_da_pagina, quadro::Quadro};
use feather_draw::exportar_svg;

fn main() -> Result<(), String> {
    let mut sessao = Sessao::default();
    sessao.inserir_tabela(3, 3).map_err(|e| format!("Falha ao inserir tabela: {e}"))?;
    let binario = sessao.quadro(0, u64::MAX).map_err(|e| format!("Falha ao montar pagina 1: {e}"))?;
    let quadro = Quadro::ler(&binario)?;
    let cena = cena_da_pagina(&quadro);
    if cena.figuras.is_empty() {
        return Err("O quadro de tabela nao produziu vetores.".into());
    }
    let svg = exportar_svg(&cena).map_err(|e| format!("SVG invalido: {e:?}"))?;
    eprintln!(
        "DRW-013: pagina {:.0}x{:.0} pt; {} vetores projetados (texto/imagem nao incluídos)",
        cena.largura, cena.altura, cena.figuras.len()
    );
    print!("{svg}");
    Ok(())
}
