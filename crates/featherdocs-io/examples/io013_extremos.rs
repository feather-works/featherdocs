//! IO-013: FDoz com tabela extensa, Unicode, revisões e comentários.
//! cargo run --manifest-path featherdocs/crates/featherdocs-io/Cargo.toml \
//!   --example io013_extremos -j 2
use featherdocs_core::{
    inline::analisar,
    modelo::{Comentario, Conteudo, Documento, Marca, Resposta, Revisao, Trecho, TipoDeRevisao},
};
use featherdocs_io::fdoz;

fn conferir(linhas: usize) -> Result<Vec<u8>, String> {
    let mut doc = Documento::vazio();
    doc.meta.titulo = "Relatório de manutenção elétrica ⚡ — 2026".into();
    doc.meta.autor = "Engenharia de Vitória/ES".into();
    doc.extras.controlar_alteracoes = true;
    doc.extras.protegido = true;
    doc.extras.marca = "REVISÃO 🔍".into();
    doc.acrescentar(Conteudo::Titulo {
        nivel: 1,
        texto: analisar("Relatório da subestação: ação, café, energia ⚡"),
    });
    let texto = vec![
        Trecho::objeto(Marca::Comentario { id: 77, fim: false }),
        Trecho {
            texto: "Novo registro 日本語".into(),
            rev: Some(Box::new(Revisao {
                tipo: TipoDeRevisao::Insercao,
                autor: "Ana".into(),
                quando: "08/10/2026".into(),
                movido: false,
                antes: None,
            })),
            ..Trecho::default()
        },
        Trecho::objeto(Marca::Comentario { id: 77, fim: true }),
    ];
    doc.acrescentar(Conteudo::Paragrafo { texto });
    doc.comentarios.push(Comentario {
        id: 77,
        autor: "Engenharia ⚡".into(),
        iniciais: String::new(),
        trechos: Vec::new(),
        quando: "08/10/2026".into(),
        texto: "Validar registros da manutenção".into(),
        resolvido: true,
        respostas: vec![Resposta {
            autor: "Auditoria".into(),
            quando: "08/10/2026".into(),
            texto: "Conferido: ação e medição".into(),
        }],
    });

    let mut tabela = Vec::with_capacity(linhas + 1);
    tabela.push(["Data", "Equipamento", "Energia", "Status", "Responsável", "Código"]
        .into_iter().map(analisar).collect());
    for i in 0..linhas {
        let valores = [
            format!("{:02}/10/2026", i % 28 + 1),
            format!("TR-{} / estação São José", i % 5),
            format!("{:.2} kWh", (i as f64 * 1.25) - 500.0),
            if i % 2 == 0 { "ação preventiva" } else { "日本語 / ⚡" }.to_owned(),
            format!("Técnico número {i}: café"),
            format!("COD-{i:06}"),
        ];
        tabela.push(valores.iter().map(|valor| analisar(valor)).collect());
    }
    doc.acrescentar(Conteudo::Tabela { cabecalho: true, linhas: tabela });
    let original = doc;

    let bytes = fdoz::gravar(&original)?;
    if !bytes.starts_with(b"PK\x03\x04") {
        return Err("FDoz não possui assinatura ZIP".into());
    }
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes.as_slice()))
        .map_err(|e| e.to_string())?;
    for nome in [
        "data/documento.arrow", "data/blocos.arrow",
        "data/trechos.arrow", "data/tabelas.arrow",
        "data/comentarios.arrow", "data/respostas.arrow",
    ] {
        let mut entrada = zip.by_name(nome).map_err(|e| e.to_string())?;
        let mut marca = [0u8; 6];
        std::io::Read::read_exact(&mut entrada, &mut marca).map_err(|e| e.to_string())?;
        if &marca != b"ARROW1" {
            return Err(format!("{nome} não é IPC Arrow"));
        }
    }
    let primeira = fdoz::ler(&bytes)?;
    if primeira != original {
        return Err("tabela Unicode, revisões ou comentários não sobreviveram à leitura".into());
    }
    let bytes2 = fdoz::gravar(&primeira)?;
    if fdoz::ler(&bytes2)? != original {
        return Err("metadados ou tabela mudaram depois da regravação".into());
    }
    if fdoz::ler(&bytes[..bytes.len() / 2]).is_ok() {
        return Err("FDoz truncado foi aceito".into());
    }
    Ok(bytes2)
}

fn main() {
    
    let destino = std::env::args().nth(1)
        .unwrap_or_else(|| "/tmp/io013_grande.fdoz".into());
    let resultado = (|| -> Result<usize, String> {
        let bytes = conferir(1_200)?;
        std::fs::write(&destino, &bytes).map_err(|e| e.to_string())?;
        let disco = std::fs::read(&destino).map_err(|e| e.to_string())?;
        if disco != bytes {
            return Err("bytes em disco diferem do buffer original".into());
        }
        fdoz::ler(&disco)?;
        Ok(bytes.len())
    })();
    match resultado {
        Ok(bytes) => println!("IO-013 FDoz OK: {bytes} bytes em {destino} (registros validados; Unicode e metadados)"),
        Err(e) => {
            eprintln!("IO-013 FDoz FALHOU: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn io013_fdoz_tabela_extensa_unicode_metadados_revisoes_e_corrupcao() {
        assert!(conferir(160).unwrap().len() > 512);
    }
}
