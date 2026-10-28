use super::*;
use featherdocs_core::inline::analisar;
use featherdocs_core::modelo::{NEGRITO, SERIFA};

#[test]
fn ida_e_volta() {
    let mut d = Documento::vazio();
    d.meta.titulo = "Meu doc".into();
    d.acrescentar(Conteudo::Titulo { nivel: 1, texto: analisar("Oi **você**") });
    d.acrescentar(Conteudo::sumario(2));
    d.acrescentar(Conteudo::Tabela { cabecalho: true, linhas: vec![vec![analisar("a"), analisar("b")]] });
    let volta = ler(&gravar(&d).unwrap()).unwrap();
    assert_eq!(volta, d);
}

#[test]
fn todo_tipo_de_bloco_volta_igual() {
    let mut d = Documento::vazio();
    let png = base64::para_data_url("image/png", &[0x89, b'P', b'N', b'G', 0, 1, 2]);
    for c in [
        Conteudo::Titulo { nivel: 3, texto: analisar("Título") },
        Conteudo::Paragrafo { texto: Vec::new() },
        Conteudo::Paragrafo { texto: analisar("Com [link](https://x.org) e `código`\tdepois da tabulação") },
        Conteudo::Item { ordenado: true, nivel: 2, texto: analisar("item") },
        Conteudo::Citacao { texto: analisar("*cita*") },
        Conteudo::Codigo { linguagem: "rust".into(), codigo: "fn main() {\n\n}".into() },
        // Linhas de tamanhos diferentes e célula vazia: a forma vem de tabelas.arrow.
        Conteudo::Tabela { cabecalho: false, linhas: vec![vec![analisar("a"), Vec::new(), analisar("c")], vec![analisar("só uma")]] },
        Conteudo::Imagem { origem: png, legenda: analisar("A *legenda*"), largura: 120.0, altura: 80.0 },
        Conteudo::Imagem { origem: "https://x.org/a.jpg".into(), legenda: Vec::new(), largura: 0.0, altura: 0.0 },
        Conteudo::Equacao { latex: "\\frac{a}{b}".into() },
        Conteudo::Sumario { niveis: 4, sem_numeros: true, sem_pontilhado: true, lista: 2, latex: true },
        Conteudo::Regua,
        Conteudo::QuebraDePagina,
        Conteudo::QuebraDeColuna,
        Conteudo::QuebraDeSecao { inicio: featherdocs_core::modelo::InicioDeSecao::PaginaImpar },
    ] {
        d.acrescentar(c);
    }
    assert_eq!(ler(&gravar(&d).unwrap()).unwrap(), d);
}

#[test]
fn formatacao_rica_sobrevive_ao_arquivo() {
    let mut d = Documento::vazio();
    let texto = vec![Trecho { texto: "oi".into(), estilo: SERIFA, fmt: Fmt { tamanho: 20.0, cor: 0xff00_00ff, realce: 0xffff_00ff, posicao: -1, ..Fmt::default() }, ..Trecho::default() }];
    d.acrescentar(Conteudo::Item { ordenado: false, nivel: 0, texto });
    d.blocos[0].atrib = Atrib {
        alinhamento: Alinhamento::Justificado,
        entrelinha: 1.5,
        antes: Some(12.0),
        depois: None,
        fundo: 0x1a3a_5cff,
        borda: 9,
        cor_borda: 0x2e6d_a4ff,
        lista: Lista::Tarefa,
        marcado: true,
        reiniciar: true,
        recuo_esq: 10.0,
        recuo_dir: 5.0,
        primeira: -8.0,
        estilo: 0,
        sem_numero: false,
        legenda: 2,
        cor_marcador: 0x2e6d_a4ff,
        apendice: true,
        encher: true,
    };
    assert_eq!(ler(&gravar(&d).unwrap()).unwrap(), d);
}

#[test]
fn pagina_estilos_e_marcadores_sobrevivem_ao_arquivo() {
    let mut d = Documento::vazio();
    d.meta = Meta { titulo: "Relatório".into(), autor: "Ana".into() };
    d.pagina = ConfigPagina { largura: 612.0, altura: 792.0, margem_sup: 36.85, margem_dir: 48.0, margem_inf: 36.85, margem_esq: 48.0 };
    d.extras = Extras {
        cabecalho: Faixas { esquerda: "{titulo}".into(), centro: String::new(), direita: "{autor}".into(), fio: 0.4, estilo: 64, tamanho: 10.0, cor: 0x112233ff, ..Faixas::default() },
        rodape: Faixas::default(),
        primeira_diferente: true,
        // A primeira página com o logo em cima e "página 1 de 2" embaixo.
        cabecalho_primeira: Faixas { imagem: "data:image/png;base64,iVBO".into(), imagem_largura: 590.5, imagem_altura: 64.25, ..Faixas::default() },
        rodape_primeira: Faixas { direita: "página {pagina} de {total}".into(), tamanho: 8.0, ..Faixas::default() },
        cor_pagina: 0xfff8_e7ff,
        marca: "RASCUNHO".into(),
        marca_cor: 0xcccc_ccff,
        borda_cor: 0x0000_80ff,
        borda_espessura: 1.5,
        borda_distancia: 24.0,
        vertical: Vertical::Centro,
        numerar_linhas: NumeracaoDeLinhas::PorPagina,
        formato_notas: 1,
        formato_notas_fim: 0,
        controlar_alteracoes: false,
        protegido: false,
        composicao_tex: true,
        pagina_inicial: 0,
    };
    d.estilos.numeracao = Numeracao::Capitulo;
    d.estilos.lista.push(Estilo { id: 1, nome: "Capa".into(), base: 1, bits: NEGRITO, tamanho: 30.0, cor: 0x3366_99ff, alinhamento: Alinhamento::Centro, antes: Some(0.0), depois: None, entrelinha: 1.0, ..Estilo::default() });
    let id = d.acrescentar(Conteudo::Titulo { nivel: 1, texto: analisar("Relatório") });
    d.blocos[0].atrib.estilo = 1;
    d.marcadores.push(Marcador { nome: "inicio".into(), bloco: id });
    assert_eq!(ler(&gravar(&d).unwrap()).unwrap(), d);
}

#[test]
fn dentro_e_arrow_e_a_imagem_e_arquivo() {
    let mut d = Documento::vazio();
    d.acrescentar(Conteudo::Imagem { origem: base64::para_data_url("image/jpeg", b"JPEG"), legenda: Vec::new(), largura: 10.0, altura: 10.0 });
    let bytes = gravar(&d).unwrap();
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes.as_slice())).unwrap();
    let nomes: Vec<String> = zip.file_names().map(String::from).collect();
    assert!(nomes.iter().all(|n| !n.ends_with(".json")), "nada de JSON: {nomes:?}");
    assert!(nomes.contains(&"data/blocos.arrow".to_string()));
    assert_eq!(ler_parte(&mut zip, "assets/imagem-1.jpg").unwrap(), b"JPEG");
}

#[test]
fn lixo_e_formato_antigo_dao_erro_em_portugues() {
    assert!(ler(b"nao e zip").unwrap_err().contains("não é um .fdoz"));
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("manifest.json", SimpleFileOptions::default()).unwrap();
    zip.write_all(br#"{"aplicativo":"FeatherDocs","formato":1}"#).unwrap();
    let antigo = zip.finish().unwrap().into_inner();
    assert!(ler(&antigo).unwrap_err().contains("formato antigo"));
}

#[test]
fn versao_futura_e_recusada() {
    let meta = HashMap::from([("formato".to_string(), "99".to_string())]);
    let doc = tabela(vec![("titulo", texto(""))], meta).unwrap();
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("data/documento.arrow", SimpleFileOptions::default()).unwrap();
    zip.write_all(&doc).unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    assert!(ler(&bytes).unwrap_err().contains("versão 99"));
}

#[test]
fn objetos_presos_ao_texto_e_notas_com_texto_voltam_iguais() {
    let mut d = Documento::vazio();
    d.extras.formato_notas = 3;
    d.extras.formato_notas_fim = 1;
    let mut texto = analisar("Antes ");
    texto.push(Trecho::objeto(Marca::Nota { fim: false, texto: analisar("A **nota** um") }));
    texto.push(Trecho::simples(" meio "));
    texto.push(Trecho::objeto(Marca::Citacao { chaves: vec!["silva2023".into(), "abnt".into()], pagina: "12".into() }));
    texto.push(Trecho::objeto(Marca::Indice { termo: "gato".into(), sub: "felino".into() }));
    texto.push(Trecho::objeto(Marca::Campo { campo: "autor".into() }));
    texto.push(Trecho::objeto(Marca::Nota { fim: true, texto: analisar("de fim") }));
    d.acrescentar(Conteudo::Paragrafo { texto });
    let mut cel = analisar("c");
    cel.push(Trecho::objeto(Marca::Nota { fim: false, texto: analisar("na célula") }));
    d.acrescentar(Conteudo::Tabela { cabecalho: false, linhas: vec![vec![analisar("a"), cel]] });
    assert_eq!(ler(&gravar(&d).unwrap()).unwrap(), d);
}

#[test]
fn fontes_estilo_e_blocos_gerados_voltam_iguais() {
    use featherdocs_core::modelo::{Autor, EstiloDeCitacao, Fonte, TipoDeFonte};
    let mut d = Documento::vazio();
    d.bibliografia.estilo = EstiloDeCitacao::Ieee;
    d.bibliografia.fontes = vec![
        Fonte { chave: "a".into(), tipo: TipoDeFonte::Artigo, autores: vec![Autor { sobrenome: "Silva".into(), nome: "Ana".into() }, Autor { sobrenome: "Lima".into(), nome: String::new() }], titulo: "T".into(), ano: "2020".into(), doi: "10/x".into(), ..Fonte::default() },
        Fonte { chave: "b".into(), tipo: TipoDeFonte::Site, url: "https://x.org".into(), acesso: "1 jan. 2024".into(), ..Fonte::default() },
    ];
    d.acrescentar(Conteudo::Bibliografia { todas: true });
    d.acrescentar(Conteudo::IndiceRemissivo);
    assert_eq!(ler(&gravar(&d).unwrap()).unwrap(), d);
}

#[test]
fn sugestoes_comentarios_e_modos_voltam_iguais() {
    use featherdocs_core::modelo::{Comentario, Marca, Resposta, Revisao, TipoDeRevisao};
    let mut d = Documento::vazio();
    d.extras.controlar_alteracoes = true;
    d.extras.protegido = true;
    let rev = |tipo| Some(Box::new(Revisao { tipo, autor: "Ana".into(), quando: "06/10/2026".into() }));
    let texto = vec![
        Trecho::objeto(Marca::Comentario { id: 7, fim: false }),
        Trecho { texto: "novo".into(), rev: rev(TipoDeRevisao::Insercao), ..Trecho::default() },
        Trecho { texto: "velho".into(), rev: rev(TipoDeRevisao::Remocao), ..Trecho::default() },
        Trecho::objeto(Marca::Comentario { id: 7, fim: true }),
    ];
    d.acrescentar(Conteudo::Paragrafo { texto });
    d.comentarios.push(Comentario { id: 7, autor: "Ana".into(), quando: "06/10/2026".into(), texto: "Veja".into(), resolvido: true, respostas: vec![Resposta { autor: "Bia".into(), quando: String::new(), texto: "Ok".into() }] });
    assert_eq!(ler(&gravar(&d).unwrap()).unwrap(), d);
}

#[test]
fn parte_que_estoura_ao_descompactar_e_recusada() {
    // 2 MiB de zeros viram poucos KiB no ZIP; o teto (1 MiB nos testes) vale para o descompactado.
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file("bomba", zip::write::SimpleFileOptions::default()).unwrap();
    std::io::Write::write_all(&mut zip, &vec![0u8; 2 << 20]).unwrap();
    zip.start_file("pequena", zip::write::SimpleFileOptions::default()).unwrap();
    std::io::Write::write_all(&mut zip, b"ok").unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes.as_slice())).unwrap();
    assert!(super::ler_parte(&mut z, "bomba").unwrap_err().contains("MiB"));
    assert_eq!(super::ler_parte(&mut z, "pequena").unwrap(), b"ok");
}
