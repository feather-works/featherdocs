//! Catraca da paridade do `.docx`: roda a mesma medição do exemplo `paridade_docx` e falha se algum
//! item piorar (um `[x]` que vira `[~]`, um `[~]` que vira `[ ]`) em relação à linha de base guardada
//! em `dados/paridade_docx_base.txt`. Melhorar passa; o teste avisa e diz como atualizar a base:
//! `PARIDADE_ATUALIZAR=1 cargo test --test paridade_docx` (a partir de `crates/featherdocs-io`).

#[path = "../examples/paridade_docx.rs"]
#[allow(dead_code)]
mod paridade;

use std::collections::HashMap;

const BASE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/dados/paridade_docx_base.txt");

/// `' '` < `'~'` < `'x'`.
fn grau(c: char) -> u8 {
    match c {
        'x' => 2,
        '~' => 1,
        _ => 0,
    }
}

/// Uma linha da base: `[x] [~] [ ] seção › recurso`.
fn linha(chave: &str, m: [char; 3]) -> String {
    format!("[{}] [{}] [{}] {chave}", m[0], m[1], m[2])
}

fn ler_base(texto: &str) -> HashMap<String, [char; 3]> {
    texto
        .lines()
        .filter(|l| l.len() > 12 && l.starts_with('['))
        .map(|l| {
            let c: Vec<char> = l.chars().collect();
            (c[12..].iter().collect(), [c[1], c[5], c[9]])
        })
        .collect()
}

#[test]
fn a_paridade_do_docx_nao_piora() {
    let itens = paridade::itens();
    let mut atual: Vec<(String, [char; 3])> = Vec::new();
    for i in &itens {
        let r = paridade::medir_protegido(i);
        atual.push((format!("{} › {}", i.secao, i.nome), [r.le.0, r.grava.0, r.volta.0]));
    }
    let mut vistas = std::collections::HashSet::new();
    for (k, _) in &atual {
        assert!(vistas.insert(k.clone()), "dois itens com o mesmo nome na medição: {k}");
    }
    let novo: String = atual.iter().map(|(k, m)| linha(k, *m) + "\n").collect();
    if std::env::var_os("PARIDADE_ATUALIZAR").is_some() {
        std::fs::write(BASE, &novo).expect("gravar a base");
        return;
    }
    let base = ler_base(&std::fs::read_to_string(BASE).expect("falta dados/paridade_docx_base.txt (gere com PARIDADE_ATUALIZAR=1)"));
    let mut pioras = Vec::new();
    let mut melhoras = 0;
    for (k, m) in &atual {
        match base.get(k) {
            Some(b) => {
                if (0..3).any(|i| grau(m[i]) < grau(b[i])) {
                    pioras.push(format!("  {}  (era {})", linha(k, *m), linha("", *b).trim_end()));
                } else if (0..3).any(|i| grau(m[i]) > grau(b[i])) {
                    melhoras += 1;
                }
            }
            None => melhoras += 1,
        }
    }
    for k in base.keys().filter(|k| !atual.iter().any(|(a, _)| a == *k)) {
        pioras.push(format!("  {k}: o item sumiu da medição"));
    }
    assert!(pioras.is_empty(), "A paridade do .docx piorou em {} item(ns):\n{}", pioras.len(), pioras.join("\n"));
    if melhoras > 0 {
        eprintln!("A paridade melhorou em {melhoras} item(ns). Atualize a base: PARIDADE_ATUALIZAR=1 cargo test --test paridade_docx");
    }
}
