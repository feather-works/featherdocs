//! CHT-016 — Gráficos do Docs: tabela nativa -> Charts -> SVG embutido.
//! O vínculo de origem fica no próprio SVG, preservado pelos codecs de imagem.
//! Sem Iced, sem materializar planilhas externas, sem alterar o esquema FDOZ.
use featherdocs_core::{
    modelo::{Conteudo, Documento, Trecho},
    sessao::{Mudanca, Sessao},
};
pub use feather_charts::Tipo;
use feather_charts::{Grafico, Serie, TipografiaGrafico};
use feather_draw::Cor;
use serde::{Deserialize, Serialize};

const PREFIXO: &str = "<!-- FeatherOfficeChart:v1:";
const SUFIXO: &str = " -->";
const LARGURA: f32 = 448.0;
const ALTURA: f32 = 252.0;
const MAX_SVG: usize = 2 * 1024 * 1024;
const MAX_CATEGORIAS: usize = 5_000;
const MAX_SERIES: usize = 16;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Vinculo {
    tabela: u64,
    titulo: String,
    tipo: String,
    /// Posição entre tabelas, independente dos IDs recriados na importação DOCX.
    /// Arquivos SVG antigos continuam funcionando: ausência significa usar só ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tabela_ordem: Option<usize>,
    /// Cabeçalhos permitem identificar a tabela correta caso DOCX renumere IDs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cabecalhos: Option<Vec<String>>,
    /// Gráfico com os dados dentro dele (sem tabela no texto): a linha 0 tem os nomes das
    /// séries, a coluna 0 as categorias. Com ela, `tabela` não é usada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    grade: Option<Vec<Vec<String>>>,
    /// O pedaço da grade que vira gráfico (`A1:D5`); ausente = a grade inteira.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    intervalo: Option<String>,
}

/// Os dados de um gráfico que os guarda dentro dele, para a janela "Dados do gráfico".
#[derive(Clone, Debug, PartialEq)]
pub struct DadosGrafico {
    pub titulo: String,
    pub tipo: Tipo,
    /// A planilha do gráfico, linha a linha (pode ter mais que o gráfico usa).
    pub grade: Vec<Vec<String>>,
    /// O pedaço da grade que vira gráfico, como `A1:D5` (vazio = a grade inteira). Dentro dele, a
    /// primeira linha tem os nomes das séries e a primeira coluna as categorias.
    pub intervalo: String,
}

/// A coluna `c` (0 = A) em letras: A…Z, AA…
pub fn letra_da_coluna(mut c: usize) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (c % 26) as u8);
        if c < 26 {
            break;
        }
        c = c / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

/// `B5` → (linha 4, coluna 1).
fn celula_a1(t: &str) -> Option<(usize, usize)> {
    let t = t.trim().trim_start_matches('$');
    let corte = t.find(|c: char| !c.is_ascii_alphabetic())?;
    let (letras, numero) = t.split_at(corte);
    if letras.is_empty() || letras.len() > 3 {
        return None;
    }
    let coluna = letras.bytes().fold(0usize, |n, b| n * 26 + (b.to_ascii_uppercase() - b'A' + 1) as usize) - 1;
    let linha: usize = numero.trim_start_matches('$').parse().ok().filter(|&n| n >= 1)?;
    Some((linha - 1, coluna))
}

/// `A2:B5` (em qualquer ordem) → os cantos (linha, coluna) de cima à esquerda e de baixo à direita.
pub fn ler_intervalo(t: &str) -> Option<((usize, usize), (usize, usize))> {
    let (a, b) = t.split_once(':').unwrap_or((t, t));
    let (a, b) = (celula_a1(a)?, celula_a1(b)?);
    Some(((a.0.min(b.0), a.1.min(b.1)), (a.0.max(b.0), a.1.max(b.1))))
}

/// Os cantos em `A1:D5`.
pub fn escrever_intervalo((l0, c0): (usize, usize), (l1, c1): (usize, usize)) -> String {
    format!("{}{}:{}{}", letra_da_coluna(c0), l0 + 1, letra_da_coluna(c1), l1 + 1)
}

/// O pedaço `intervalo` da grade (o que falta fora da grade vem vazio); intervalo vazio = tudo.
fn recorte(grade: &[Vec<String>], intervalo: &str) -> Result<Vec<Vec<String>>, String> {
    if intervalo.trim().is_empty() {
        return Ok(grade.to_vec());
    }
    let ((l0, c0), (l1, c1)) = ler_intervalo(intervalo).ok_or("Intervalo inválido: use algo como A1:D5.")?;
    if l1 - l0 > MAX_CATEGORIAS || c1 - c0 > MAX_SERIES {
        return Err("O intervalo passa de 5000 categorias ou 16 séries.".into());
    }
    Ok((l0..=l1).map(|l| (c0..=c1).map(|c| grade.get(l).and_then(|r| r.get(c)).cloned().unwrap_or_default()).collect()).collect())
}

impl DadosGrafico {
    /// Os dados de exemplo de um gráfico novo, como no Word.
    pub fn exemplo(tipo: Tipo) -> Self {
        let linha = |c: &str, v: [&str; 3]| [c].into_iter().chain(v).map(String::from).collect();
        DadosGrafico {
            titulo: "Título do gráfico".into(),
            tipo,
            grade: vec![
                linha("", ["Série 1", "Série 2", "Série 3"]),
                linha("Categoria 1", ["4,3", "2,4", "2"]),
                linha("Categoria 2", ["2,5", "4,4", "2"]),
                linha("Categoria 3", ["3,5", "1,8", "3"]),
                linha("Categoria 4", ["4,5", "2,8", "5"]),
            ],
            intervalo: "A1:D5".into(),
        }
    }
}

fn nome_tipo(tipo: Tipo) -> &'static str {
    match tipo {
        Tipo::Colunas => "colunas",
        Tipo::Barras => "barras",
        Tipo::Linhas => "linhas",
        Tipo::Area => "area",
        Tipo::Degraus => "degraus",
        Tipo::Pizza => "pizza",
        Tipo::Dispersao => "dispersao",
    }
}

fn ler_tipo(nome: &str) -> Option<Tipo> {
    Some(match nome {
        "colunas" => Tipo::Colunas,
        "barras" => Tipo::Barras,
        "linhas" => Tipo::Linhas,
        "area" => Tipo::Area,
        "degraus" => Tipo::Degraus,
        "pizza" => Tipo::Pizza,
        "dispersao" => Tipo::Dispersao,
        _ => return None,
    })
}

fn texto(celula: &[Trecho]) -> String {
    celula.iter().map(|trecho| trecho.texto.as_str()).collect::<String>().trim().to_string()
}

fn numero(celula: &str) -> Option<f64> {
    let limpo = celula.trim().replace(' ', "");
    if limpo.is_empty() {
        return None;
    }
    let decimal = if limpo.contains(',') {
        // PT-BR: pontos separam milhar e vírgula marca os decimais.
        limpo.replace('.', "").replace(',', ".")
    } else {
        limpo
    };
    decimal.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Resolve a tabela exata pelo ID. Se a importação DOCX recriou os IDs,
/// recupera-a pela ordem das tabelas (não pela posição geral de blocos).
fn cabecalhos_da_tabela(bloco: &featherdocs_core::modelo::Bloco) -> Option<Vec<String>> {
    let Conteudo::Tabela { cabecalho: true, linhas } = &bloco.conteudo else {
        return None;
    };
    Some(linhas.first()?.iter().map(|celula| texto(celula)).collect())
}

fn tabela_da_origem<'a>(
    doc: &'a Documento,
    vinculo: &Vinculo,
) -> Result<&'a featherdocs_core::modelo::Bloco, String> {
    let por_id = doc.blocos.iter().find(|b| {
        b.id == vinculo.tabela && matches!(b.conteudo, Conteudo::Tabela { .. })
    });
    let por_ordem = vinculo.tabela_ordem.and_then(|ordem| {
        doc.blocos.iter()
            .filter(|b| matches!(b.conteudo, Conteudo::Tabela { .. }))
            .nth(ordem)
    });
    let combina = |bloco: &featherdocs_core::modelo::Bloco| {
        vinculo.cabecalhos.as_deref() == cabecalhos_da_tabela(bloco).as_deref()
    };
    match (por_id, por_ordem) {
        (Some(origem), Some(ordinal)) if origem.id != ordinal.id && vinculo.cabecalhos.is_some() => {
            match (combina(origem), combina(ordinal)) {
                (true, false) => Ok(origem), // tabela reordenada no Docs: ID estável
                (false, true) => Ok(ordinal), // DOCX reatribuiu o ID
                _ => Err("Vínculo ambíguo: duas tabelas não podem ser diferenciadas com segurança.".into()),
            }
        }
        (Some(origem), _) => Ok(origem), // mantém compatibilidade com SVG v1 antigo
        (None, Some(ordinal)) if vinculo.cabecalhos.is_none() || combina(ordinal) => Ok(ordinal),
        (None, Some(_)) => Err("A tabela na posição original tem outro cabeçalho; não vou trocar a fonte do gráfico.".into()),
        _ => Err("A tabela de origem do gráfico não existe mais.".into()),
    }
}

fn dados(doc: &Documento, vinculo: &Vinculo) -> Result<Grafico, String> {
    if let Some(grade) = &vinculo.grade {
        return da_grade(&recorte(grade, vinculo.intervalo.as_deref().unwrap_or(""))?, vinculo, true);
    }
    let bloco = tabela_da_origem(doc, vinculo)?;
    let Conteudo::Tabela { cabecalho: true, linhas } = &bloco.conteudo else {
        return Err("A origem do gráfico deve ser uma tabela com cabeçalho.".into());
    };
    let grade: Vec<Vec<String>> = linhas.iter().map(|l| l.iter().map(|c| texto(c)).collect()).collect();
    da_grade(&grade, vinculo, false)
}

/// O gráfico de uma grade de textos. `vazio_zero`: célula de valor vazia conta 0 (na janela de
/// dados, enquanto se digita); na tabela do texto ela é erro.
fn da_grade(linhas: &[Vec<String>], vinculo: &Vinculo, vazio_zero: bool) -> Result<Grafico, String> {
    if !(2..=MAX_CATEGORIAS + 1).contains(&linhas.len()) {
        return Err("O gráfico precisa de 1 a 5000 categorias.".into());
    }
    let colunas = linhas[0].len();
    if !(2..=MAX_SERIES + 1).contains(&colunas)
        || linhas.iter().any(|linha| linha.len() != colunas)
    {
        return Err("A tabela do gráfico deve ser retangular e ter até 16 séries.".into());
    }
    let mut categorias = Vec::with_capacity(linhas.len() - 1);
    let mut series: Vec<Serie> = (1..colunas).map(|i| {
        let paleta = [
            Cor::rgb(44, 117, 171), Cor::rgb(228, 115, 62),
            Cor::rgb(45, 155, 113), Cor::rgb(147, 91, 175),
            Cor::rgb(223, 165, 45), Cor::rgb(55, 153, 178),
        ];
        Serie {
            nome: linhas[0][i].trim().to_string(),
            valores: Vec::with_capacity(linhas.len() - 1),
            cor: paleta[(i - 1) % paleta.len()],
        }
    }).collect();
    for linha in linhas.iter().skip(1) {
        let categoria = linha[0].trim().to_string();
        if categoria.is_empty() {
            return Err("Há uma categoria vazia na tabela.".into());
        }
        categorias.push(categoria);
        for (indice, serie) in series.iter_mut().enumerate() {
            let celula = &linha[indice + 1];
            let valor = (if vazio_zero && celula.trim().is_empty() { Some(0.0) } else { numero(celula) })
                .ok_or_else(|| format!("Valor inválido na categoria {} e série {}.", categorias.len(), indice + 1))?;
            serie.valores.push(valor);
        }
    }
    let grafico = Grafico {
        titulo: vinculo.titulo.clone(),
        tipo: ler_tipo(&vinculo.tipo).ok_or("Tipo de gráfico desconhecido.")?,
        categorias,
        series,
    };
    grafico.validar().map_err(|e| format!("Dados inválidos para o gráfico: {e:?}"))?;
    Ok(grafico)
}

fn renderizar(doc: &Documento, vinculo: &Vinculo) -> Result<String, String> {
    // Se DOCX recriou os IDs, persiste o novo ID válido no próprio SVG.
    let mut atual = vinculo.clone();
    if vinculo.grade.is_none() {
        let origem = tabela_da_origem(doc, vinculo)?;
        atual.tabela = origem.id;
        atual.tabela_ordem = doc.blocos.iter()
            .filter(|b| matches!(b.conteudo, Conteudo::Tabela { .. }))
            .position(|b| b.id == origem.id);
        atual.cabecalhos = cabecalhos_da_tabela(origem);
    }
    let grafico = dados(doc, &atual)?;
    let mut svg = grafico.exportar_svg_anotado(LARGURA, ALTURA, &TipografiaGrafico::default())
        .map_err(|e| format!("Falha ao renderizar gráfico: {e:?}"))?;
    let json = serde_json::to_vec(&atual).map_err(|e| e.to_string())?;
    let comentario = format!("{PREFIXO}{}{SUFIXO}", crate::base64::codificar(&json));
    let fim = svg.rfind("</svg>").ok_or("O motor Charts não produziu SVG válido.")?;
    svg.insert_str(fim, &comentario);
    if svg.len() > MAX_SVG {
        return Err("O SVG do gráfico ultrapassou o limite de 2 MiB.".into());
    }
    Ok(crate::base64::para_data_url("image/svg+xml", svg.as_bytes()))
}

fn vinculo_da_imagem(origem: &str) -> Option<Vinculo> {
    if origem.len() > MAX_SVG * 2 {
        return None;
    }
    let (mime, bytes) = crate::base64::de_data_url(origem)?;
    if mime != "image/svg+xml" || bytes.len() > MAX_SVG {
        return None;
    }
    let svg = std::str::from_utf8(&bytes).ok()?;
    let inicio = svg.find(PREFIXO)? + PREFIXO.len();
    let resto = svg.get(inicio..)?;
    let final_marca = resto.find(SUFIXO)?;
    if final_marca > MAX_SVG {
        return None;
    }
    let dado = crate::base64::decodificar(&resto[..final_marca])?;
    let vinculo: Vinculo = serde_json::from_slice(&dado).ok()?;
    ler_tipo(&vinculo.tipo)?;
    Some(vinculo)
}

/// Renderiza tabela nativa do FeatherDocs em qualquer um dos 25 tipos Rust.
///
/// API comum de exportação/prévia para novos consumidores; não muda
/// inserção/vínculo SVG legado nem o formato FDOZ/DOCX.
pub fn exportar_tabela_motor(
    documento: &Documento, tabela_id: u64,
    tipo: feather_charts::motor::TipoGrafico,
    largura: f32, altura: f32,
) -> Result<String, String> {
    use feather_charts::motor::{renderizar_svg, Entrada};
    let bloco = documento.blocos.iter()
        .find(|b| b.id == tabela_id && matches!(b.conteudo, Conteudo::Tabela { .. }))
        .ok_or("Tabela do gráfico não encontrada.")?;
    let tabela_ordem = documento.blocos.iter()
        .filter(|b| matches!(b.conteudo, Conteudo::Tabela { .. }))
        .position(|b| b.id == tabela_id);
    let vinculo = Vinculo {
        tabela: tabela_id, titulo: "Gráfico da tabela".into(),
        tipo: "colunas".into(), tabela_ordem,
        cabecalhos: cabecalhos_da_tabela(bloco),
        grade: None,
        intervalo: None,
    };
    let grafico = dados(documento, &vinculo)?;
    let entrada = Entrada {
        titulo: grafico.titulo, categorias: grafico.categorias,
        series: grafico.series, x_values: None, z_values: None,
    };
    renderizar_svg(tipo, &entrada, largura, altura)
        .map_err(|e| format!("Motor Charts: {e:?}"))
}

/// Insere um gráfico vinculado à tabela onde o cursor está.
/// Dados numéricos são lidos de células reais do Docs, sem Iced.
pub fn inserir_da_tabela(sessao: &mut Sessao, tipo: Tipo) -> Result<Mudanca, String> {
    let tabela = sessao.cursor().foco.bloco;
    let nome = sessao.documento().blocos.iter().find(|b| b.id == tabela)
        .ok_or("Selecione uma tabela para inserir um gráfico.")?;
    if !matches!(nome.conteudo, Conteudo::Tabela { .. }) {
        return Err("Coloque o cursor em uma tabela para inserir o gráfico.".into());
    }
    let tabela_ordem = sessao.documento().blocos.iter()
        .filter(|b| matches!(b.conteudo, Conteudo::Tabela { .. }))
        .position(|b| b.id == tabela);
    let vinculo = Vinculo {
        tabela,
        titulo: "Gráfico da tabela".into(),
        tipo: nome_tipo(tipo).to_string(),
        tabela_ordem,
        cabecalhos: cabecalhos_da_tabela(nome),
        grade: None,
        intervalo: None,
    };
    let url = renderizar(sessao.documento(), &vinculo)?;
    sessao.inserir_imagem(&url, LARGURA, ALTURA)
}

/// Atualiza os valores sem trocar o ID do bloco de imagem e preservando undo/redo.
pub fn atualizar_no_cursor(sessao: &mut Sessao) -> Result<Mudanca, String> {
    let id = sessao.cursor().foco.bloco;
    let bloco = sessao.documento().blocos.iter().find(|b| b.id == id)
        .ok_or("O gráfico selecionado não foi encontrado.")?;
    let Conteudo::Imagem { origem, .. } = &bloco.conteudo else {
        return Err("Selecione o gráfico embutido para atualizar.".into());
    };
    let vinculo = vinculo_da_imagem(origem)
        .ok_or("A imagem não é um gráfico editável do FeatherWorks.")?;
    let nova_origem = renderizar(sessao.documento(), &vinculo)?;
    sessao.imagem_origem_atualizar(id, &nova_origem)
}

/// Insere no cursor um gráfico com os dados dentro dele (os de exemplo) e devolve o id do bloco
/// da imagem, para abrir a janela "Dados do gráfico".
pub fn inserir_com_dados(sessao: &mut Sessao, dados: &DadosGrafico) -> Result<u64, String> {
    let url = renderizar(sessao.documento(), &vinculo_de(dados))?;
    sessao.inserir_imagem(&url, LARGURA, ALTURA)?;
    sessao.documento().blocos.iter().rev()
        .find(|b| matches!(&b.conteudo, Conteudo::Imagem { origem, .. } if *origem == url))
        .map(|b| b.id)
        .ok_or_else(|| "O gráfico inserido não foi encontrado.".into())
}

/// Os dados do gráfico `origem`, se ele os guarda dentro dele (os ligados a uma tabela, não).
pub fn dados_do_grafico(origem: &str) -> Option<DadosGrafico> {
    let v = vinculo_da_imagem(origem)?;
    Some(DadosGrafico { titulo: v.titulo, tipo: ler_tipo(&v.tipo)?, grade: v.grade?, intervalo: v.intervalo.unwrap_or_default() })
}

/// Troca os dados do gráfico no bloco `id` e o redesenha (um passo de desfazer).
pub fn definir_dados(sessao: &mut Sessao, id: u64, dados: &DadosGrafico) -> Result<Mudanca, String> {
    let url = renderizar(sessao.documento(), &vinculo_de(dados))?;
    sessao.imagem_origem_atualizar(id, &url)
}

fn vinculo_de(dados: &DadosGrafico) -> Vinculo {
    Vinculo {
        tabela: 0,
        titulo: dados.titulo.clone(),
        tipo: nome_tipo(dados.tipo).into(),
        tabela_ordem: None,
        cabecalhos: None,
        grade: Some(dados.grade.clone()),
        intervalo: (!dados.intervalo.is_empty()).then(|| dados.intervalo.clone()),
    }
}

/// Identifica gráficos Docs sem modificar imagens SVG comuns.
pub fn eh_grafico(origem: &str) -> bool {
    vinculo_da_imagem(origem).is_some()
}

#[cfg(test)]
mod testes {
    use super::*;
    use featherdocs_core::modelo::{Documento, Trecho};

    fn trecho(s: &str) -> Vec<Trecho> {
        vec![Trecho { texto: s.to_string(), ..Trecho::default() }]
    }

    fn documento() -> Documento {
        let mut d = Documento::vazio();
        d.acrescentar(Conteudo::Tabela {
            cabecalho: true,
            linhas: vec![
                vec![trecho("Categoria"), trecho("Consumo"), trecho("Meta")],
                vec![trecho("Jan"), trecho("12,5"), trecho("10")],
                vec![trecho("Fev"), trecho("15"), trecho("13")],
            ],
        });
        d
    }

    #[test]
    fn insere_e_fdoz_preserva_fonte_e_svg() {
        let mut sessao = Sessao::com_documento(documento());
        inserir_da_tabela(&mut sessao, Tipo::Colunas).unwrap();
        let antes = sessao.documento().clone();
        let imagem = antes.blocos.iter().find_map(|b| {
            if let Conteudo::Imagem { origem, .. } = &b.conteudo { Some(origem.as_str()) } else { None }
        }).unwrap();
        assert!(eh_grafico(imagem));
        let fdoz = crate::fdoz::gravar(&antes).unwrap();
        let depois = crate::fdoz::ler(&fdoz).unwrap();
        assert_eq!(antes, depois);
        let imagem = depois.blocos.iter().find_map(|b| {
            if let Conteudo::Imagem { origem, .. } = &b.conteudo { Some(origem.as_str()) } else { None }
        }).unwrap();
        assert!(eh_grafico(imagem));
    }

    #[test]
    fn numeros_invalidos_rejeitados_sem_inserir() {
        let mut d = documento();
        if let Conteudo::Tabela { linhas, .. } = &mut d.blocos[0].conteudo {
            linhas[1][1] = trecho("erro");
        }
        let mut sessao = Sessao::com_documento(d);
        let original = sessao.documento().clone();
        assert!(inserir_da_tabela(&mut sessao, Tipo::Colunas).is_err());
        assert_eq!(*sessao.documento(), original);
    }

    #[test]
    fn svg_generico_nao_e_editavel() {
        let url = crate::base64::para_data_url("image/svg+xml", b"<svg></svg>");
        assert!(!eh_grafico(&url));
        assert!(!eh_grafico("https://site.com/grafico.svg"));
    }

    #[test]
    fn atualizacao_preserva_id_e_desfazer_refazer() {
        let mut sessao = Sessao::com_documento(documento());
        inserir_da_tabela(&mut sessao, Tipo::Linhas).unwrap();
        let mut doc = sessao.documento().clone();
        let id = doc.blocos.iter().find(|b| matches!(b.conteudo, Conteudo::Imagem { .. })).unwrap().id;
        let antiga = match &doc.blocos.iter().find(|b| b.id == id).unwrap().conteudo {
            Conteudo::Imagem { origem, .. } => origem.clone(),
            _ => unreachable!(),
        };
        if let Conteudo::Tabela { linhas, .. } = &mut doc.blocos[0].conteudo {
            linhas[1][1] = trecho("200,5");
        }
        let mut editada = Sessao::com_documento(doc);
        editada.ir_para_bloco(id);
        atualizar_no_cursor(&mut editada).unwrap();
        let atual = match &editada.documento().blocos.iter().find(|b| b.id == id).unwrap().conteudo {
            Conteudo::Imagem { origem, .. } => origem.clone(),
            _ => unreachable!(),
        };
        assert_ne!(antiga, atual);
        assert!(eh_grafico(&atual));
        editada.desfazer().unwrap();
        let desfeita = match &editada.documento().blocos.iter().find(|b| b.id == id).unwrap().conteudo {
            Conteudo::Imagem { origem, .. } => origem.clone(),
            _ => unreachable!(),
        };
        assert_eq!(desfeita, antiga);
        editada.refazer().unwrap();
        let refeita = match &editada.documento().blocos.iter().find(|b| b.id == id).unwrap().conteudo {
            Conteudo::Imagem { origem, .. } => origem.as_str(),
            _ => unreachable!(),
        };
        assert_eq!(refeita, atual);
    }

    #[test]
    fn atualizacao_rejeita_fonte_desaparecida() {
        let mut doc = documento();
        let v = Vinculo { tabela: 9999, titulo: "Teste".into(), tipo: "colunas".into(), tabela_ordem: None, cabecalhos: None, grade: None, intervalo: None };
        let url = crate::base64::para_data_url(
            "image/svg+xml",
            format!("<svg>{PREFIXO}{}{SUFIXO}</svg>", crate::base64::codificar(&serde_json::to_vec(&v).unwrap())).as_bytes(),
        );
        doc.acrescentar(Conteudo::Imagem { origem: url, legenda: Vec::new(), largura: 440.0, altura: 250.0 });
        let sessao = Sessao::com_documento(doc);
        // O adaptador rejeita referência inválida antes de editar o documento.
        let imagem = sessao.documento().blocos.last().unwrap();
        let Conteudo::Imagem { origem, .. } = &imagem.conteudo else { unreachable!() };
        let v = vinculo_da_imagem(origem).unwrap();
        assert!(renderizar(sessao.documento(), &v).is_err());
    }

    #[test]
    fn docx_preserva_grafico_e_permite_atualizacao_depois_de_reabrir() {
        let mut sessao = Sessao::com_documento(documento());
        inserir_da_tabela(&mut sessao, Tipo::Barras).unwrap();
        let arquivo = crate::docx::gravar(sessao.documento()).unwrap();
        let recuperado = crate::docx::ler(&arquivo).unwrap();
        let origem = recuperado.blocos.iter().find_map(|b| match &b.conteudo {
            Conteudo::Imagem { origem, .. } => Some((b.id, origem)),
            _ => None,
        }).expect("DOCX deve preservar a imagem do gráfico");
        assert!(eh_grafico(origem.1), "DOCX perdeu o SVG e o vínculo da tabela");
        let id = origem.0;
        let mut reaberta = Sessao::com_documento(recuperado);
        reaberta.ir_para_bloco(id);
        atualizar_no_cursor(&mut reaberta).expect("DOCX deve permitir atualizar o gráfico");
        let guardado = crate::fdoz::gravar(reaberta.documento()).unwrap();
        let lido = crate::fdoz::ler(&guardado).unwrap();
        assert!(lido.blocos.iter().any(|b| {
            matches!(&b.conteudo, Conteudo::Imagem { origem, .. } if eh_grafico(origem))
        }));
    }

    #[test]
    fn fonte_renumerada_no_docx_e_religada_sem_perder_o_svg() {
        let mut sessao = Sessao::com_documento(documento());
        inserir_da_tabela(&mut sessao, Tipo::Barras).unwrap();
        let mut doc = sessao.documento().clone();
        let id_grafico = doc.blocos.iter().find(|b| matches!(&b.conteudo, Conteudo::Imagem { .. }))
            .unwrap().id;
        let tabela = doc.blocos.iter_mut().find(|b| matches!(&b.conteudo, Conteudo::Tabela { .. })).unwrap();
        tabela.id = 300;
        doc.proximo_id = 301;
        let mut reaberta = Sessao::com_documento(doc);
        reaberta.ir_para_bloco(id_grafico);
        atualizar_no_cursor(&mut reaberta).unwrap();
        let url = reaberta.documento().blocos.iter().find_map(|b| match &b.conteudo {
            Conteudo::Imagem { origem, .. } => Some(origem),
            _ => None,
        }).unwrap();
        let vinculo = vinculo_da_imagem(url).unwrap();
        assert_eq!(vinculo.tabela, 300, "SVG deve guardar o ID atualizado");
        assert_eq!(vinculo.tabela_ordem, Some(0));
        assert_eq!(vinculo.cabecalhos.as_ref().unwrap(), &vec![
            "Categoria".to_string(), "Consumo".to_string(), "Meta".to_string()
        ]);
    }

    #[test]
    fn tabela_reordenada_mantem_vinculo_pelo_id_e_cabecalho() {
        let mut sessao = Sessao::com_documento(documento());
        inserir_da_tabela(&mut sessao, Tipo::Area).unwrap();
        let mut doc = sessao.documento().clone();
        // Uma tabela inserida ANTES da fonte muda o ordinal, mas não o ID.
        let mut outra = Documento::vazio();
        outra.acrescentar(Conteudo::Tabela {
            cabecalho: true,
            linhas: vec![
                vec![trecho("X"), trecho("Y")],
                vec![trecho("1"), trecho("2")],
            ],
        });
        let mut nova_tabela = outra.blocos.remove(0);
        nova_tabela.id = 500;
        doc.blocos.insert(0, nova_tabela);
        doc.proximo_id = 501;
        let id_grafico = doc.blocos.iter().find(|b| matches!(&b.conteudo, Conteudo::Imagem { .. }))
            .unwrap().id;
        let mut reaberta = Sessao::com_documento(doc);
        reaberta.ir_para_bloco(id_grafico);
        atualizar_no_cursor(&mut reaberta).unwrap();
        let url = reaberta.documento().blocos.iter().find_map(|b| match &b.conteudo {
            Conteudo::Imagem { origem, .. } => Some(origem),
            _ => None,
        }).unwrap();
        assert_eq!(vinculo_da_imagem(url).unwrap().tabela, 1);
    }

    #[test]
    fn vinculo_svg_antigo_sem_metadados_novos_ainda_e_legivel() {
        let doc = documento();
        // O SVG v1 antigo só codificava tabela/título/tipo.
        let v: Vinculo = serde_json::from_value(serde_json::json!({
            "tabela": doc.blocos[0].id,
            "titulo": "Legado",
            "tipo": "colunas"
        })).unwrap();
        assert!(v.tabela_ordem.is_none());
        assert!(v.cabecalhos.is_none());
        let url = renderizar(&doc, &v).unwrap();
        assert!(eh_grafico(&url));
        let lido = vinculo_da_imagem(&url).unwrap();
        assert_eq!(lido.tabela, v.tabela);
        assert_eq!(lido.tabela_ordem, Some(0), "atualização migra o vínculo antigo");
        assert_eq!(lido.cabecalhos.unwrap(), vec![
            "Categoria".to_string(), "Consumo".to_string(), "Meta".to_string()
        ]);
    }

    #[test]
    fn tabela_renumerada_com_cabecalho_diferente_nao_e_ligada_silenciosamente() {
        let mut sessao = Sessao::com_documento(documento());
        inserir_da_tabela(&mut sessao, Tipo::Linhas).unwrap();
        let mut doc = sessao.documento().clone();
        let id_grafico = doc.blocos.iter().find(|b| matches!(&b.conteudo, Conteudo::Imagem { .. }))
            .unwrap().id;
        let tabela = doc.blocos.iter_mut()
            .find(|b| matches!(&b.conteudo, Conteudo::Tabela { .. })).unwrap();
        tabela.id = 300;
        if let Conteudo::Tabela { linhas, .. } = &mut tabela.conteudo {
            linhas[0][1] = trecho("Série não relacionada");
        }
        doc.proximo_id = 301;
        let mut reaberta = Sessao::com_documento(doc);
        reaberta.ir_para_bloco(id_grafico);
        let antes = reaberta.documento().clone();
        let erro = atualizar_no_cursor(&mut reaberta).unwrap_err();
        assert!(erro.contains("cabeçalho"), "{erro}");
        assert_eq!(*reaberta.documento(), antes, "atualização inválida não pode perder dados");
    }

    #[test]
    fn sete_tipos_de_grafico_na_mesma_tabela_sem_perder_dados() {
        let tipos = [
            Tipo::Colunas, Tipo::Barras, Tipo::Linhas, Tipo::Area,
            Tipo::Pizza, Tipo::Dispersao, Tipo::Degraus,
        ];
        for tipo in tipos {
            let mut fonte = documento();
            if let Conteudo::Tabela { linhas, .. } = &mut fonte.blocos[0].conteudo {
                if tipo == Tipo::Pizza {
                    // Pizza exige uma única série, por contrato do motor Charts.
                    for linha in linhas.iter_mut() { linha.truncate(2); }
                }
                if tipo == Tipo::Dispersao {
                    // Dispersão exige coordenadas X numéricas (não meses).
                    linhas[1][0] = trecho("1");
                    linhas[2][0] = trecho("2");
                }
            }
            let mut sessao = Sessao::com_documento(fonte);
            inserir_da_tabela(&mut sessao, tipo).unwrap();
            assert!(sessao.documento().blocos.iter().any(|b| {
                matches!(&b.conteudo, Conteudo::Imagem { origem, .. } if eh_grafico(origem))
            }), "tipo {tipo:?} não criou gráfico editável");
        }
    }
    #[test]
    fn motor_compartilhado_gera_heatmap_de_tabela_docs_sem_mutar_doc() {
        let documento = documento();
        let original = documento.clone();
        let svg = exportar_tabela_motor(
            &documento, documento.blocos[0].id,
            feather_charts::motor::TipoGrafico::MapaCalor,
            448., 252.
        ).unwrap();
        assert!(svg.contains("<svg"));
        assert!(!svg.contains("NaN"));
        assert_eq!(documento, original);
    }

    #[test]
    fn grafico_com_dados_dentro_insere_le_e_troca() {
        let mut sessao = Sessao::com_documento(Documento::vazio());
        let exemplo = DadosGrafico::exemplo(Tipo::Colunas);
        let id = inserir_com_dados(&mut sessao, &exemplo).unwrap();
        let origem = |s: &Sessao| match &s.documento().blocos.iter().find(|b| b.id == id).unwrap().conteudo {
            Conteudo::Imagem { origem, .. } => origem.clone(),
            _ => panic!("não é imagem"),
        };
        assert_eq!(dados_do_grafico(&origem(&sessao)), Some(exemplo.clone()));
        // Valor vazio conta zero enquanto se digita; texto que não é número é recusado.
        let mut novo = exemplo.clone();
        novo.grade[1][1] = String::new();
        novo.tipo = Tipo::Linhas;
        definir_dados(&mut sessao, id, &novo).unwrap();
        assert_eq!(dados_do_grafico(&origem(&sessao)), Some(novo.clone()));
        novo.grade[2][2] = "abc".into();
        assert!(definir_dados(&mut sessao, id, &novo).is_err());
        // O gráfico ligado a uma tabela não tem dados próprios.
        assert!(dados_do_grafico("data:image/svg+xml;base64,PHN2Zy8+").is_none());
    }


    #[test]
    fn intervalo_a1_escolhe_o_pedaco_da_grade() {
        assert_eq!(ler_intervalo("b5:A2"), Some(((1, 0), (4, 1))));
        assert_eq!(ler_intervalo("$C$3"), Some(((2, 2), (2, 2))));
        assert_eq!(ler_intervalo("5A:B"), None);
        assert_eq!(letra_da_coluna(0), "A");
        assert_eq!(letra_da_coluna(27), "AB");
        assert_eq!(escrever_intervalo((0, 0), (4, 3)), "A1:D5");
        let mut sessao = Sessao::com_documento(Documento::vazio());
        let mut d = DadosGrafico::exemplo(Tipo::Colunas);
        let id = inserir_com_dados(&mut sessao, &d).unwrap();
        // Só as duas primeiras séries e três categorias; texto fora do intervalo não atrapalha.
        d.grade[1].push("anotação".into());
        d.intervalo = "A1:C4".into();
        definir_dados(&mut sessao, id, &d).unwrap();
        d.intervalo = "zz".into();
        assert!(definir_dados(&mut sessao, id, &d).is_err());
    }

}
