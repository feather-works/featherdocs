//! O modo dividido: o código (Markdown ou LaTeX) editado à esquerda volta para os blocos.
//!
//! O código inteiro é relido, e cada bloco lido é casado com os de antes pela "impressão digital"
//! (o código daquele bloco sozinho). Bloco cuja impressão não mudou **volta o mesmo**, com a
//! formatação que Markdown e LaTeX não sabem guardar (cor, fonte, estilo com nome, alinhamento…).
//! Os que mudaram, na mesma posição e do mesmo tipo, guardam o `id` e o que é do parágrafo, e
//! pegam o conteúdo novo; o resto entra como bloco novo (`id` 0). Quem aplica é `Sessao::trocar_blocos`.

use std::mem::discriminant;

use featherdocs_core::modelo::{Alinhamento, Atrib, Bloco, ConfigPagina, Conteudo, Documento, Extras, Lista, Meta, Numeracao};

use crate::exportar::{bloco_markdown, bloco_tex};
use crate::{latex, markdown};

/// A língua do código à esquerda.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fonte {
    Markdown,
    Latex,
}

impl Fonte {
    pub fn do_nome(nome: &str) -> Result<Self, String> {
        match nome {
            "md" => Ok(Self::Markdown),
            "tex" => Ok(Self::Latex),
            outro => Err(format!("O modo dividido não conhece o formato \"{outro}\".")),
        }
    }
}

/// O que o código pede: os blocos (os de antes com o `id` deles, os novos com `id` 0), e o título,
/// o autor e a numeração quando o código os diz (só o LaTeX diz).
#[derive(Debug)]
pub struct Reconciliado {
    pub blocos: Vec<Bloco>,
    pub meta: Option<Meta>,
    pub numeracao: Option<Numeracao>,
    /// O papel e as margens que o código descreve (LaTeX com `geometry` ou `a4paper`/`letterpaper`);
    /// `None` = o código não diz, a página fica como está.
    pub pagina: Option<ConfigPagina>,
    /// O que vale para a página e o código diz (cabeçalho, rodapé, cor, marca d'água…), sobre o que
    /// o documento tinha; `None` = nada mudou.
    pub extras: Option<Extras>,
    /// O que o leitor não converteu (só no LaTeX), com a linha no código.
    pub avisos: Vec<crate::latex::Aviso>,
}

/// Acima disto (blocos de antes × blocos novos no trecho que mudou), não se procura o que ficou
/// igual no meio: troca-se o trecho todo. Só acontece colando um documento inteiro por cima.
const LIMITE_LCS: usize = 4_000_000;

fn digital(fonte: Fonte, b: &Bloco, numeracao: Numeracao) -> String {
    match fonte {
        Fonte::Markdown => bloco_markdown(&b.conteudo),
        Fonte::Latex => bloco_tex(b, numeracao),
    }
}

/// Os pares `(i, j)` de blocos iguais (velho `i`, novo `j`), em ordem: o começo e o fim iguais, e
/// a maior subsequência comum no meio.
fn casar(velhos: &[String], novos: &[String]) -> Vec<(usize, usize)> {
    let p = velhos.iter().zip(novos).take_while(|(a, b)| a == b).count();
    let maximo = velhos.len().min(novos.len()) - p;
    let s = velhos.iter().rev().zip(novos.iter().rev()).take(maximo).take_while(|(a, b)| a == b).count();
    let mut pares: Vec<(usize, usize)> = (0..p).map(|k| (k, k)).collect();
    let (v, n) = (&velhos[p..velhos.len() - s], &novos[p..novos.len() - s]);
    if !v.is_empty() && !n.is_empty() && v.len() * n.len() <= LIMITE_LCS {
        // Tabela da maior subsequência comum, de trás para a frente.
        let largura = n.len() + 1;
        let mut t = vec![0u32; (v.len() + 1) * largura];
        for i in (0..v.len()).rev() {
            for j in (0..n.len()).rev() {
                t[i * largura + j] = if v[i] == n[j] { t[(i + 1) * largura + j + 1] + 1 } else { t[(i + 1) * largura + j].max(t[i * largura + j + 1]) };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < v.len() && j < n.len() {
            if v[i] == n[j] {
                pares.push((p + i, p + j));
                i += 1;
                j += 1;
            } else if t[(i + 1) * largura + j] >= t[i * largura + j + 1] {
                i += 1;
            } else {
                j += 1;
            }
        }
    }
    pares.extend((0..s).map(|k| (velhos.len() - s + k, novos.len() - s + k)));
    pares
}

/// O bloco que mudou no código, no lugar de um de antes do mesmo tipo: o `id` e o que é do
/// parágrafo ficam; o conteúdo (e o que o código sabe dizer do parágrafo) vem do código.
fn herdar(velho: &Bloco, novo: &Bloco, fonte: Fonte) -> Bloco {
    let mut atrib = velho.atrib;
    if novo.atrib.lista != Lista::Padrao {
        atrib.lista = novo.atrib.lista;
        atrib.marcado = novo.atrib.marcado;
    }
    if fonte == Fonte::Latex {
        atrib.sem_numero = novo.atrib.sem_numero;
        // O que o leitor tira do código (fundo da `\colorbox`, `center`, espaços da `minipage`,
        // recuos…) vale o do código; o que o código não diz (atributo no padrão) fica o de antes.
        // Sem isto, um bloco novo no lugar de um velho (colar o .tex num documento vazio) herdava
        // os atributos do vizinho: o nome branco da faixa sumia no branco, a faixa se partia.
        let n = novo.atrib;
        let padrao = Atrib::default();
        atrib.fundo = n.fundo;
        if matches!(n.alinhamento, Alinhamento::Centro | Alinhamento::Direita) {
            atrib.alinhamento = n.alinhamento;
        }
        if n.entrelinha != padrao.entrelinha { atrib.entrelinha = n.entrelinha; }
        if n.antes.is_some() { atrib.antes = n.antes; }
        if n.depois.is_some() { atrib.depois = n.depois; }
        if n.borda != padrao.borda { atrib.borda = n.borda; atrib.cor_borda = n.cor_borda; }
        if n.recuo_esq != padrao.recuo_esq { atrib.recuo_esq = n.recuo_esq; }
        if n.recuo_dir != padrao.recuo_dir { atrib.recuo_dir = n.recuo_dir; }
        if n.primeira != padrao.primeira { atrib.primeira = n.primeira; }
    }
    // A imagem embutida no documento aparece no código só pelo nome (`figura-7.png`, ou o do `.tex`
    // aberto): o nome no código não traz os bytes, então a embutida fica.
    let mut conteudo = novo.conteudo.clone();
    if let (Conteudo::Imagem { origem: o_velho, largura: l_velha, altura: a_velha, .. }, Conteudo::Imagem { origem, largura, altura, .. }) = (&velho.conteudo, &mut conteudo) {
        if o_velho.starts_with("data:") && !origem.starts_with("data:") {
            origem.clone_from(o_velho);
            // A altura saiu dos pixels do arquivo, que o código não traz: segue a proporção de antes.
            if *altura <= 0.0 && *a_velha > 0.0 && *l_velha > 0.0 {
                if *largura <= 0.0 {
                    *largura = *l_velha;
                }
                *altura = *largura * a_velha / l_velha;
            }
        }
    }
    // O editor de código não representa mesclagens. Conserva-as quando a
    // estrutura da tabela permanece idêntica, sem descartar formato por engano.
    let mesclagens = match (&velho.conteudo, &novo.conteudo) {
        (Conteudo::Tabela { linhas: a, .. }, Conteudo::Tabela { linhas: b, .. })
            if a.len() == b.len()
                && a.iter().zip(b).all(|(a, b)| a.len() == b.len()) =>
            velho.mesclagens.clone(),
        _ => Vec::new(),
    };
    // Os fios do LaTeX valem enquanto as linhas forem as mesmas.
    let latex = match (&velho.conteudo, &novo.conteudo) {
        (Conteudo::Tabela { linhas: a, .. }, Conteudo::Tabela { linhas: b, .. }) if a.len() == b.len() => velho.latex.clone(),
        _ => String::new(),
    };
    // As propriedades da tabela valem enquanto a forma for a mesma.
    let tabela = match (&velho.conteudo, &conteudo) {
        (Conteudo::Tabela { linhas: a, .. }, Conteudo::Tabela { linhas: b, .. })
            if a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.len() == b.len()) => velho.tabela.clone(),
        _ => Default::default(),
    };
    let secao = if matches!(conteudo, Conteudo::QuebraDeSecao { .. }) { velho.secao.clone() } else { None };
    let imagem = novo.imagem.clone();
    Bloco { id: velho.id, conteudo, atrib, mesclagens, latex, tabela, secao, imagem }
}

/// O código de volta para os blocos, guardando o que não mudou.
pub fn reconciliar(antigo: &Documento, fonte: Fonte, codigo: &str) -> Reconciliado {
    let (lido, avisos) = match fonte {
        Fonte::Markdown => (markdown::ler(codigo), Vec::new()),
        // `\maketitle` vira título e autor na leitura de um arquivo; aqui eles já são a meta do documento.
        Fonte::Latex => {
            let l = latex::ler_com_fontes(&codigo.replace("\\maketitle", ""), &antigo.bibliografia.fontes);
            (l.documento, l.avisos)
        }
    };
    let numeracao_nova = if fonte == Fonte::Latex { lido.estilos.numeracao } else { antigo.estilos.numeracao };
    let velhos: Vec<String> = antigo.blocos.iter().map(|b| digital(fonte, b, antigo.estilos.numeracao)).collect();
    let novos: Vec<String> = lido.blocos.iter().map(|b| digital(fonte, b, numeracao_nova)).collect();
    let pares = casar(&velhos, &novos);

    let mut blocos = Vec::with_capacity(lido.blocos.len());
    let (mut i, mut j) = (0, 0);
    for &(pi, pj) in pares.iter().chain(std::iter::once(&(antigo.blocos.len(), lido.blocos.len()))) {
        // Entre dois pares iguais: os de antes `[i, pi)` e os novos `[j, pj)` mudaram; casam pela posição.
        for k in j..pj {
            let velho = antigo.blocos.get(i + (k - j)).filter(|_| i + (k - j) < pi);
            let novo = &lido.blocos[k];
            blocos.push(match velho {
                Some(v) if discriminant(&v.conteudo) == discriminant(&novo.conteudo) => herdar(v, novo, fonte),
                _ => Bloco { id: 0, ..novo.clone() },
            });
        }
        if pi < antigo.blocos.len() {
            let mut igual = antigo.blocos[pi].clone();
            // A estrela (`\section*`) diz se o título fica fora da numeração; a impressão digital é a
            // mesma com e sem a marca quando a numeração do documento muda junto.
            if fonte == Fonte::Latex {
                igual.atrib.sem_numero = lido.blocos[pj].atrib.sem_numero;
            }
            blocos.push(igual);
        }
        (i, j) = (pi + 1, pj + 1);
    }
    let meta = (fonte == Fonte::Latex && lido.meta != antigo.meta).then(|| lido.meta.clone());
    let numeracao = (fonte == Fonte::Latex).then_some(numeracao_nova);
    let preambulo = codigo.split("\\begin{document}").next().unwrap_or("");
    let diz_a_pagina = ["{geometry}", "\\geometry", "a4paper", "letterpaper"].iter().any(|m| preambulo.contains(m));
    let pagina = (fonte == Fonte::Latex && diz_a_pagina).then_some(lido.pagina);
    let extras = (fonte == Fonte::Latex).then(|| {
        // Só o que o leitor sabe tirar do código; a borda da página, a revisão e a proteção ficam.
        let (l, mut e) = (&lido.extras, antigo.extras.clone());
        e.cabecalho.clone_from(&l.cabecalho);
        e.rodape.clone_from(&l.rodape);
        e.primeira_diferente = l.primeira_diferente;
        e.cor_pagina = l.cor_pagina;
        e.marca.clone_from(&l.marca);
        e.marca_cor = l.marca_cor;
        e.vertical = l.vertical;
        e.numerar_linhas = l.numerar_linhas;
        e.formato_notas = l.formato_notas;
        e
    }).filter(|e| *e != antigo.extras);
    Reconciliado { blocos, meta, numeracao, pagina, extras, avisos }
}

/// Duas páginas iguais a menos do arredondamento do código (as medidas vão em cm com duas casas).
pub fn mesma_pagina(a: &ConfigPagina, b: &ConfigPagina) -> bool {
    [(a.largura, b.largura), (a.altura, b.altura), (a.margem_sup, b.margem_sup), (a.margem_dir, b.margem_dir), (a.margem_inf, b.margem_inf), (a.margem_esq, b.margem_esq)]
        .iter()
        .all(|(x, y)| (x - y).abs() < 0.3)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::exportar;
    use featherdocs_core::modelo::{Alinhamento, Conteudo};

    fn colorido() -> Documento {
        let mut d = markdown::ler("# Título\n\nUm parágrafo.\n\nOutro parágrafo.\n\n- item a\n- item b\n");
        // O que o Markdown não guarda: cor no título, alinhamento no segundo parágrafo.
        d.blocos[0].atrib.alinhamento = Alinhamento::Centro;
        if let Conteudo::Titulo { texto, .. } = &mut d.blocos[0].conteudo {
            texto[0].fmt.cor = 0xff00_00ff;
        }
        d.blocos[2].atrib.alinhamento = Alinhamento::Direita;
        d
    }

    #[test]
    fn colar_tex_num_documento_vazio_traz_fundo_e_alinhamento_do_codigo() {
        let tex = "\\documentclass{article}\n\\usepackage{xcolor}\n\\definecolor{darkblue}{HTML}{1A3A5C}\n\\begin{document}\n\\noindent\n\\colorbox{darkblue}{%\n  \\begin{minipage}{\\textwidth}\n    \\centering\n    \\textbf{\\LARGE\\color{white}Nome}\\\\[3pt]\n    {\\color{white}Cargo}\n  \\end{minipage}%\n}\\par\nTexto.\n\\end{document}\n";
        let vazio = Documento::vazio();
        let r = reconciliar(&vazio, Fonte::Latex, tex);
        let lido = crate::latex::ler(tex);
        assert_eq!(r.blocos[0].atrib.fundo, lido.blocos[0].atrib.fundo);
        assert_ne!(r.blocos[0].atrib.fundo, 0, "o primeiro bloco (no lugar do parágrafo vazio) leva a faixa");
        assert_eq!(r.blocos[0].atrib.alinhamento, Alinhamento::Centro);
    }

    #[test]
    fn casar_acha_o_que_ficou_igual_no_meio() {
        let v: Vec<String> = ["a", "b", "c", "d", "e"].map(String::from).to_vec();
        let n: Vec<String> = ["a", "x", "c", "y", "e"].map(String::from).to_vec();
        assert_eq!(casar(&v, &n), [(0, 0), (2, 2), (4, 4)]);
    }

    #[test]
    fn exportar_e_reler_sem_mexer_nao_muda_nada() {
        for fonte in [Fonte::Markdown, Fonte::Latex] {
            let d = colorido();
            let codigo = if fonte == Fonte::Markdown { exportar::markdown(&d) } else { exportar::latex(&d) };
            let r = reconciliar(&d, fonte, &codigo);
            assert_eq!(r.blocos, d.blocos, "{fonte:?}: o código intacto devolve os mesmos blocos");
        }
    }

    #[test]
    fn so_o_paragrafo_editado_muda_e_os_outros_guardam_a_formatacao() {
        let d = colorido();
        let codigo = exportar::markdown(&d).replace("Um parágrafo.", "Um parágrafo editado.");
        let r = reconciliar(&d, Fonte::Markdown, &codigo);
        assert_eq!(r.blocos[0], d.blocos[0], "o título colorido e centralizado fica");
        assert_eq!(r.blocos[2], d.blocos[2]);
        assert_eq!(r.blocos[1].id, d.blocos[1].id, "o editado guarda o id");
        assert!(matches!(&r.blocos[1].conteudo, Conteudo::Paragrafo { texto } if texto[0].texto.contains("editado")));
    }

    #[test]
    fn paragrafo_novo_entra_com_id_zero() {
        let d = colorido();
        let codigo = exportar::markdown(&d).replace("Outro parágrafo.", "Novo.\n\nOutro parágrafo.");
        let r = reconciliar(&d, Fonte::Markdown, &codigo);
        assert_eq!(r.blocos.len(), d.blocos.len() + 1);
        assert_eq!(r.blocos.iter().filter(|b| b.id == 0).count(), 1);
        assert_eq!(r.blocos[3], d.blocos[2], "o seguinte continua o mesmo (com o alinhamento à direita)");
    }

    #[test]
    fn latex_traz_titulo_e_numeracao() {
        let mut d = colorido();
        d.meta.titulo = "Relatório".into();
        let codigo = exportar::latex(&d).replace("\\section*{", "\\section{").replace("\\title{Relatório}", "\\title{Relatório final}");
        let r = reconciliar(&d, Fonte::Latex, &codigo);
        assert_eq!(r.numeracao, Some(Numeracao::Decimal));
        assert_eq!(r.meta.unwrap().titulo, "Relatório final");
        assert_eq!(r.blocos.len(), d.blocos.len(), "o \\maketitle não vira bloco");
        assert_eq!(r.blocos[0].atrib.alinhamento, Alinhamento::Centro, "o título mudou de \\section* para \\section e guardou o alinhamento");
    }

    #[test]
    fn todo_tipo_de_bloco_volta_igual_do_codigo_intacto() {
        let md = "# Um\n\n## Dois **negrito** e *itálico* e `código` e [link](https://x.y)\n\nTexto com $x^2$ e ~~tachado~~.\n\n- a\n  - a1\n- b\n\n1. um\n2. dois\n\n- [ ] tarefa\n- [x] feita\n\n> citação\n\n```rust\nfn main() {}\n```\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n\n$$\nE = mc^2\n$$\n\n![legenda](img.png)\n\n---\n\n[TOC]\n\n[LOF]\n\n[LOT]\n";
        let d = markdown::ler(md);
        for fonte in [Fonte::Markdown, Fonte::Latex] {
            let codigo = if fonte == Fonte::Markdown { exportar::markdown(&d) } else { exportar::latex(&d) };
            let r = reconciliar(&d, fonte, &codigo);
            let mudaram: Vec<String> = r.blocos.iter().zip(&d.blocos).filter(|(a, b)| a != b).map(|(a, b)| format!("{:?}\n  virou {:?}", b.conteudo, a.conteudo)).collect();
            assert!(r.blocos.len() == d.blocos.len() && mudaram.is_empty(), "{fonte:?}: {} vs {} blocos\n{}", r.blocos.len(), d.blocos.len(), mudaram.join("\n"));
        }
    }

    #[test]
    fn titulo_com_estrela_fica_fora_quando_a_numeracao_liga_pelo_codigo() {
        let d = markdown::ler("# Capa\n\ntexto\n");
        let codigo = exportar::latex(&d).replace("texto", "texto\n\n\\section{Um}");
        let r = reconciliar(&d, Fonte::Latex, &codigo);
        assert_eq!(r.numeracao, Some(Numeracao::Decimal));
        assert!(r.blocos[0].atrib.sem_numero, "a capa (\\section*) não ganha número");
        assert!(!r.blocos[2].atrib.sem_numero);
    }

    #[test]
    fn margens_vao_para_o_latex_e_voltam_dele() {
        let mut d = markdown::ler("Texto.\n");
        d.pagina = ConfigPagina { largura: 841.89, altura: 595.28, margem_sup: 85.04, margem_dir: 56.69, margem_inf: 56.69, margem_esq: 85.04 };
        let codigo = exportar::latex(&d);
        assert!(codigo.contains("\\usepackage[a4paper, landscape, top=3.00cm, bottom=2.00cm, left=3.00cm, right=2.00cm]{geometry}"), "{codigo}");
        let r = reconciliar(&Documento::default(), Fonte::Latex, &codigo);
        assert!(mesma_pagina(&r.pagina.expect("o código diz a página"), &d.pagina), "{:?}", r.pagina);
        // Sem geometry no código, a página do documento não muda.
        assert!(reconciliar(&d, Fonte::Latex, "\\documentclass{article}\n\\begin{document}\nTexto.\n\\end{document}").pagina.is_none());
    }

    #[test]
    fn layout_e_formatacao_vao_para_o_latex_e_voltam() {
        use featherdocs_core::modelo::{Faixas, Fmt, NumeracaoDeLinhas, Trecho, Vertical};
        let mut d = markdown::ler("Primeiro parágrafo.\n\nSegundo parágrafo.\n\nTerceiro.\n");
        d.extras.cabecalho = Faixas { direita: "{pagina}".into(), ..Faixas::default() };
        d.extras.rodape = Faixas { esquerda: "Relatório".into(), ..Faixas::default() };
        d.extras.cor_pagina = 0xFFF8E7FF;
        d.extras.marca = "RASCUNHO".into();
        d.extras.numerar_linhas = NumeracaoDeLinhas::Continua;
        d.extras.primeira_diferente = true;
        d.extras.vertical = Vertical::Centro;
        d.blocos[1].atrib.alinhamento = Alinhamento::Centro;
        d.blocos[2].atrib.alinhamento = Alinhamento::Justificado;
        d.blocos[2].atrib.antes = Some(12.0);
        if let Conteudo::Paragrafo { texto } = &mut d.blocos[0].conteudo {
            texto.push(Trecho { texto: " marcado".into(), fmt: Fmt { realce: 0xFFFF00FF, ..Fmt::default() }, ..Trecho::default() });
            texto.push(Trecho { texto: "2".into(), fmt: Fmt { posicao: 1, ..Fmt::default() }, ..Trecho::default() });
        }
        let codigo = exportar::latex(&d);
        let lido = crate::latex::ler(&codigo);
        // O texto e o fio voltam; a letra é a do corpo do LaTeX.
        let sem_letra = |f: &featherdocs_core::modelo::Faixas| (f.esquerda.clone(), f.centro.clone(), f.direita.clone(), f.fio);
        assert_eq!(sem_letra(&lido.extras.cabecalho), sem_letra(&d.extras.cabecalho), "{codigo}");
        assert_eq!(sem_letra(&lido.extras.rodape), sem_letra(&d.extras.rodape));
        assert_eq!((lido.extras.cor_pagina, lido.extras.marca.as_str(), lido.extras.numerar_linhas), (0xFFF8E7FF, "RASCUNHO", NumeracaoDeLinhas::Continua));
        assert!(lido.extras.primeira_diferente);
        assert_eq!(lido.extras.vertical, Vertical::Centro);
        let alinhamentos: Vec<_> = lido.blocos.iter().map(|b| b.atrib.alinhamento).collect();
        assert_eq!(alinhamentos, [Alinhamento::Esquerda, Alinhamento::Centro, Alinhamento::Justificado], "{codigo}");
        assert_eq!(lido.blocos[2].atrib.antes, Some(12.0));
        let Conteudo::Paragrafo { texto } = &lido.blocos[0].conteudo else { panic!() };
        assert_eq!(featherdocs_core::inline::texto_puro(texto), "Primeiro parágrafo. marcado2");
        assert!(texto.iter().any(|t| t.texto.contains("marcado") && t.fmt.realce == 0xFFFF00FF), "{texto:?}");
        assert!(texto.iter().any(|t| t.texto == "2" && t.fmt.posicao == 1));
    }
}

