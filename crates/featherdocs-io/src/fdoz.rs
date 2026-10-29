//! O `.fdoz`: contêiner ZIP (contrato geral em `FORMATO.md`) com o documento em **Arrow**
//! colunar, como o `.fshz` do FeatherSheet — nada de JSON.
//!
//! ```text
//! data/documento.arrow   uma linha: título, autor, papel, margens, cabeçalho, rodapé,
//!                        fundo e borda da página, numeração (formato e aplicativo nos metadados do esquema)
//! data/blocos.arrow      um bloco por linha: tipo, conteúdo próprio (código, fórmula, imagem…) e atributos
//! data/trechos.arrow     o texto formatado: um trecho por linha, apontando o bloco (e a célula, em tabela)
//! data/tabelas.arrow     a forma das tabelas: quantas células tem cada linha
//! data/estilos.arrow     os estilos com nome (de parágrafo e de caractere; a linha de id 0 é o
//!                        padrão do documento, e o tema do Word vai nos metadados, "tema")
//! data/marcadores.arrow  os marcadores (nome → bloco)
//! data/fontes.arrow      as fontes citadas (uma por linha; falta nos arquivos de antes dela)
//! data/comentarios.arrow os comentários, e data/respostas.arrow as respostas deles
//! data/listas.arrow      as listas do Word (um nível por linha, apontando a lista; falta nos
//!                        arquivos de antes da fase 3 do .docx)
//! data/sacola.arrow      as partes do pacote .docx dos objetos do Word guardados (uma por linha,
//!                        apontando o trecho; falta nos arquivos de antes dela)
//! assets/                as imagens, como arquivo (não como texto base64)
//! ```
//!
//! O `.git/` embutido do contrato **ainda não entrou**. O `.fdoz` antigo (JSON, formato 1, só da
//! fase de testes) não abre mais — o mesmo corte que o FeatherSheet fez com o `.oplz`.

use std::collections::HashMap;
use std::io::{Cursor, Write};
use std::sync::Arc;

use arrow::array::{Array, ArrayRef, BinaryArray, BooleanArray, Float32Array, Int8Array, RecordBatch, StringArray, UInt32Array, UInt64Array, UInt8Array};
use arrow::datatypes::{Field, Schema};
use arrow::ipc::reader::FileReader;
use arrow::ipc::writer::FileWriter;
use featherdocs_core::modelo::{Alinhamento, Atrib, Autor, Bloco, Comentario, Resposta, Revisao, TipoDeRevisao, EstiloDeCitacao, Fonte, TipoDeFonte, Celula, ConfigPagina, Conteudo, Documento, MesclagemTabela, Estilo, Estilos, Extras, Faixas, Familia, Fmt, InicioDeSecao, Lista, ListaDoWord, Marca, Marcador, Meta, NivelDeLista, Numeracao, NumeracaoDeLinhas, ParteWord, Trecho, Vertical};
use zip::write::SimpleFileOptions;

use crate::base64;

/// A versão do formato: 2 = Arrow (1 era o JSON da fase de testes).
const VERSAO: u32 = 2;
const APLICATIVO: &str = "FeatherDocs";

fn erro(e: &dyn std::fmt::Display) -> String {
    format!("Não consegui gravar o .fdoz: {e}")
}

// ---------------------------------------------------------------------------
// Códigos dos enums (o que vai para as colunas `u8`)
// ---------------------------------------------------------------------------

const ALINHAMENTOS: [Alinhamento; 4] = [Alinhamento::Esquerda, Alinhamento::Centro, Alinhamento::Direita, Alinhamento::Justificado];
const LISTAS: [Lista; 9] = [Lista::Padrao, Lista::Circulo, Lista::Quadrado, Lista::Seta, Lista::Tarefa, Lista::Numerica, Lista::Alfabetica, Lista::Romana, Lista::RomanaMinuscula];
const VERTICAIS: [Vertical; 3] = [Vertical::Topo, Vertical::Centro, Vertical::Base];
const LINHAS_NUMERADAS: [NumeracaoDeLinhas; 3] = [NumeracaoDeLinhas::Nenhuma, NumeracaoDeLinhas::Continua, NumeracaoDeLinhas::PorPagina];
const NUMERACOES: [Numeracao; 3] = [Numeracao::Nenhuma, Numeracao::Decimal, Numeracao::Capitulo];
const ESTILOS_DE_CITACAO: [EstiloDeCitacao; 5] = [EstiloDeCitacao::Abnt, EstiloDeCitacao::Apa, EstiloDeCitacao::Vancouver, EstiloDeCitacao::Ieee, EstiloDeCitacao::Chicago];
const TIPOS_DE_FONTE: [TipoDeFonte; 7] = [TipoDeFonte::Livro, TipoDeFonte::Capitulo, TipoDeFonte::Artigo, TipoDeFonte::Site, TipoDeFonte::Tese, TipoDeFonte::Relatorio, TipoDeFonte::Outro];
/// Separa os autores (e, dentro de um autor, o sobrenome do nome) na coluna `autores`.
const ENTRE_AUTORES: char = '\u{1e}';
const ENTRE_NOMES: char = '\u{1f}';

fn codigo_de<T: PartialEq>(tabela: &[T], v: &T) -> u8 {
    tabela.iter().position(|x| x == v).unwrap_or(0) as u8
}

fn valor_de<T: Copy>(tabela: &[T], c: u8) -> T {
    tabela.get(usize::from(c)).copied().unwrap_or(tabela[0])
}

/// O tipo do bloco na coluna `tipo`.
fn tipo_do_bloco(c: &Conteudo) -> u8 {
    match c {
        Conteudo::Titulo { .. } => 0,
        Conteudo::Paragrafo { .. } => 1,
        Conteudo::Item { .. } => 2,
        Conteudo::Citacao { .. } => 3,
        Conteudo::Codigo { .. } => 4,
        Conteudo::Tabela { .. } => 5,
        Conteudo::Imagem { .. } => 6,
        Conteudo::Equacao { .. } => 7,
        Conteudo::Sumario { .. } => 8,
        Conteudo::Regua => 9,
        Conteudo::QuebraDePagina => 10,
        Conteudo::Bibliografia { .. } => 11,
        Conteudo::IndiceRemissivo => 12,
        Conteudo::QuebraDeColuna => 13,
        Conteudo::QuebraDeSecao { .. } => 14,
    }
}

// ---------------------------------------------------------------------------
// Tabelas Arrow: gravar e ler
// ---------------------------------------------------------------------------

/// Uma tabela Arrow (IPC, formato de arquivo) com as colunas dadas.
fn tabela(colunas: Vec<(&str, ArrayRef)>, metadados: HashMap<String, String>) -> Result<Vec<u8>, String> {
    let campos: Vec<Field> = colunas.iter().map(|(n, a)| Field::new(*n, a.data_type().clone(), true)).collect();
    let esquema = Arc::new(Schema::new_with_metadata(campos, metadados));
    let lote = RecordBatch::try_new(esquema.clone(), colunas.into_iter().map(|(_, a)| a).collect()).map_err(|e| erro(&e))?;
    let mut bytes = Vec::new();
    let mut w = FileWriter::try_new(&mut bytes, &esquema).map_err(|e| erro(&e))?;
    w.write(&lote).map_err(|e| erro(&e))?;
    w.finish().map_err(|e| erro(&e))?;
    drop(w);
    Ok(bytes)
}

/// Uma tabela lida: as colunas pelo nome e os metadados do esquema.
struct Lida {
    lote: RecordBatch,
    nome: &'static str,
}

impl Lida {
    fn de(bytes: &[u8], nome: &'static str) -> Result<Self, String> {
        let leitor = FileReader::try_new(Cursor::new(bytes), None).map_err(|e| format!("{nome} ilegível: {e}"))?;
        let esquema = leitor.schema();
        let lotes: Vec<RecordBatch> = leitor.collect::<Result<_, _>>().map_err(|e| format!("{nome} ilegível: {e}"))?;
        let lote = arrow::compute::concat_batches(&esquema, &lotes).map_err(|e| format!("{nome} ilegível: {e}"))?;
        Ok(Self { lote, nome })
    }

    fn linhas(&self) -> usize {
        self.lote.num_rows()
    }

    fn meta(&self, chave: &str) -> Option<&str> {
        self.lote.schema_ref().metadata().get(chave).map(String::as_str)
    }

    fn col<T: 'static>(&self, campo: &str) -> Result<&T, String> {
        self.lote.column_by_name(campo).and_then(|c| c.as_any().downcast_ref::<T>()).ok_or_else(|| format!("{}: falta a coluna \"{campo}\".", self.nome))
    }
}

/// O resto do `w:rPr`/`w:pPr` do Word (`MaisLetra`, `MaisParagrafo`) numa coluna de texto opcional,
/// em JSON: nulo = o padrão (a coluna falta nos arquivos de antes da fase 2 do .docx).
fn em_json<T: serde::Serialize + Default + PartialEq>(v: &T) -> Option<String> {
    (*v != T::default()).then(|| serde_json::to_string(v).unwrap_or_default())
}

fn de_json<T: serde::de::DeserializeOwned + Default>(t: &Lida, coluna: &str, i: usize) -> T {
    t.col::<StringArray>(coluna).ok().and_then(|a| opcional(a, i, |a, i| serde_json::from_str(a.value(i)).ok())).flatten().unwrap_or_default()
}

/// Valor de uma coluna que aceita nulo.
fn opcional<A: Array, T>(a: &A, i: usize, f: impl Fn(&A, usize) -> T) -> Option<T> {
    (!a.is_null(i)).then(|| f(a, i))
}

// ---------------------------------------------------------------------------
// Gravar
// ---------------------------------------------------------------------------

/// Grava o documento num `.fdoz`.
pub fn gravar(doc: &Documento) -> Result<Vec<u8>, String> {
    let mut imagens = Vec::new();
    let (trechos, sacola) = trechos_arrow(&doc.blocos)?;
    let mut partes: Vec<(String, Vec<u8>)> = vec![
        ("data/documento.arrow".into(), documento_arrow(doc)?),
        ("data/blocos.arrow".into(), blocos_arrow(&doc.blocos, &mut imagens)?),
        ("data/trechos.arrow".into(), trechos),
        ("data/sacola.arrow".into(), sacola),
        ("data/tabelas.arrow".into(), tabelas_arrow(&doc.blocos)?),
        ("data/estilos.arrow".into(), estilos_arrow(&doc.estilos)?),
        ("data/listas.arrow".into(), listas_arrow(&doc.estilos.listas)?),
        ("data/marcadores.arrow".into(), marcadores_arrow(&doc.marcadores)?),
        ("data/fontes.arrow".into(), fontes_arrow(&doc.bibliografia.fontes)?),
        ("data/comentarios.arrow".into(), comentarios_arrow(&doc.comentarios)?),
        ("data/respostas.arrow".into(), respostas_arrow(&doc.comentarios)?),
    ];
    partes.extend(imagens);
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opcoes = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (nome, conteudo) in partes {
        zip.start_file(nome, opcoes).map_err(|e| erro(&e))?;
        zip.write_all(&conteudo).map_err(|e| erro(&e))?;
    }
    Ok(zip.finish().map_err(|e| erro(&e))?.into_inner())
}

fn texto(v: &str) -> ArrayRef {
    Arc::new(StringArray::from(vec![v]))
}
fn u32_(v: u32) -> ArrayRef {
    Arc::new(UInt32Array::from(vec![v]))
}
fn f32_(v: f32) -> ArrayRef {
    Arc::new(Float32Array::from(vec![v]))
}
fn u8_(v: u8) -> ArrayRef {
    Arc::new(UInt8Array::from(vec![v]))
}

fn documento_arrow(doc: &Documento) -> Result<Vec<u8>, String> {
    let (p, x) = (&doc.pagina, &doc.extras);
    let meta = HashMap::from([("aplicativo".to_string(), APLICATIVO.to_string()), ("formato".to_string(), VERSAO.to_string())]);
    tabela(
        vec![
            ("titulo", texto(&doc.meta.titulo)),
            ("autor", texto(&doc.meta.autor)),
            ("largura", f32_(p.largura)),
            ("altura", f32_(p.altura)),
            ("margem_sup", f32_(p.margem_sup)),
            ("margem_dir", f32_(p.margem_dir)),
            ("margem_inf", f32_(p.margem_inf)),
            ("margem_esq", f32_(p.margem_esq)),
            ("proximo_id", Arc::new(UInt64Array::from(vec![doc.proximo_id]))),
            ("cabecalho_esq", texto(&x.cabecalho.esquerda)),
            ("cabecalho_centro", texto(&x.cabecalho.centro)),
            ("cabecalho_dir", texto(&x.cabecalho.direita)),
            ("rodape_esq", texto(&x.rodape.esquerda)),
            ("rodape_centro", texto(&x.rodape.centro)),
            ("rodape_dir", texto(&x.rodape.direita)),
            ("faixas_fmt", texto(&[&x.cabecalho, &x.rodape, &x.cabecalho_primeira, &x.rodape_primeira].map(|f| format!("{} {} {} {:x}", f.fio, f.estilo, f.tamanho, f.cor)).join(";"))),
            ("cabecalho1_esq", texto(&x.cabecalho_primeira.esquerda)),
            ("cabecalho1_centro", texto(&x.cabecalho_primeira.centro)),
            ("cabecalho1_dir", texto(&x.cabecalho_primeira.direita)),
            ("rodape1_esq", texto(&x.rodape_primeira.esquerda)),
            ("rodape1_centro", texto(&x.rodape_primeira.centro)),
            ("rodape1_dir", texto(&x.rodape_primeira.direita)),
            // As imagens das quatro faixas, na mesma ordem do `faixas_fmt`, separadas por linha.
            ("faixas_imagens", texto(&[&x.cabecalho, &x.rodape, &x.cabecalho_primeira, &x.rodape_primeira].map(|f| format!("{} {} {}", f.imagem_largura, f.imagem_altura, f.imagem)).join("\n"))),
            ("primeira_diferente", Arc::new(BooleanArray::from(vec![x.primeira_diferente]))),
            ("cor_pagina", u32_(x.cor_pagina)),
            ("marca", texto(&x.marca)),
            ("marca_cor", u32_(x.marca_cor)),
            ("borda_cor", u32_(x.borda_cor)),
            ("borda_espessura", f32_(x.borda_espessura)),
            ("borda_distancia", f32_(x.borda_distancia)),
            ("vertical", u8_(codigo_de(&VERTICAIS, &x.vertical))),
            ("numerar_linhas", u8_(codigo_de(&LINHAS_NUMERADAS, &x.numerar_linhas))),
            ("numeracao", u8_(codigo_de(&NUMERACOES, &doc.estilos.numeracao))),
            ("formato_notas", u8_(x.formato_notas)),
            ("formato_notas_fim", u8_(x.formato_notas_fim)),
            ("estilo_citacao", u8_(codigo_de(&ESTILOS_DE_CITACAO, &doc.bibliografia.estilo))),
            ("controlar_alteracoes", Arc::new(BooleanArray::from(vec![x.controlar_alteracoes]))),
            ("protegido", Arc::new(BooleanArray::from(vec![x.protegido]))),
            ("composicao_tex", Arc::new(BooleanArray::from(vec![x.composicao_tex]))),
            ("pagina_inicial", Arc::new(UInt32Array::from(vec![x.pagina_inicial]))),
            // As decorações presas à página, em JSON (vazio = nenhuma; a coluna falta nos arquivos de antes).
            ("decoracoes", texto(&if x.decoracoes.is_empty() { String::new() } else { serde_json::to_string(&x.decoracoes).unwrap_or_default() })),
            ("documento", texto(&if x.documento.eh_padrao() { String::new() } else { serde_json::to_string(&x.documento).unwrap_or_default() })),
            // A tinta de cada página (`FTINTA`), separadas pelo separador de registro.
            ("tinta", texto(&x.tinta.join("\u{1e}"))),
            ("sem_sl_sn", Arc::new(BooleanArray::from(vec![doc.bibliografia.sem_sl_sn]))),
            // Os blocos das quatro faixas (as do `faixas_fmt`) e a assinatura de quando foram lidos.
            ("faixas_blocos", Arc::new(StringArray::from(vec![em_json(&[&x.cabecalho, &x.rodape, &x.cabecalho_primeira, &x.rodape_primeira].map(|f| (f.blocos.clone(), f.lida.clone())))]))),
        ],
        meta,
    )
}

/// As colunas da tabela de blocos, uma `Vec` por coluna.
#[derive(Default)]
struct ColunasDeBloco {
    id: Vec<u64>,
    tipo: Vec<u8>,
    nivel: Vec<u8>,
    ordenado: Vec<bool>,
    fonte: Vec<Option<String>>,
    linguagem: Vec<Option<String>>,
    largura: Vec<f32>,
    altura: Vec<f32>,
    /// Cabeçalho da tabela; no sumário, "sem números".
    marca_a: Vec<bool>,
    /// No sumário, "sem pontilhado".
    marca_b: Vec<bool>,
    /// No sumário, 0 = títulos, 1 = figuras, 2 = tabelas.
    lista_sumario: Vec<u8>,
    mesclagens: Vec<String>,
    latex: Vec<String>,
    /// As propriedades de tabela do Word (`PropsTabela`) em JSON; nulo = nenhuma.
    tabela: Vec<Option<String>>,
    /// A seção que termina na quebra (`Secao`) em JSON; nulo = a do documento.
    secao: Vec<Option<String>>,
    /// As propriedades da imagem do Word (`PropsImagem`) em JSON; nulo = nenhuma.
    imagem: Vec<Option<String>>,
    a: ColunasDeAtrib,
}

#[derive(Default)]
struct ColunasDeAtrib {
    alinhamento: Vec<u8>,
    entrelinha: Vec<f32>,
    antes: Vec<Option<f32>>,
    depois: Vec<Option<f32>>,
    fundo: Vec<u32>,
    borda: Vec<u8>,
    cor_borda: Vec<u32>,
    lista: Vec<u8>,
    marcado: Vec<bool>,
    reiniciar: Vec<bool>,
    recuo_esq: Vec<f32>,
    recuo_dir: Vec<f32>,
    primeira: Vec<f32>,
    estilo: Vec<u32>,
    sem_numero: Vec<bool>,
    legenda: Vec<u8>,
    cor_marcador: Vec<u32>,
    apendice: Vec<bool>,
    encher: Vec<bool>,
    mais: Vec<Option<String>>,
}

impl ColunasDeAtrib {
    fn por(&mut self, a: &Atrib) {
        self.alinhamento.push(codigo_de(&ALINHAMENTOS, &a.alinhamento));
        self.entrelinha.push(a.entrelinha);
        self.antes.push(a.antes);
        self.depois.push(a.depois);
        self.fundo.push(a.fundo);
        self.borda.push(a.borda);
        self.cor_borda.push(a.cor_borda);
        self.lista.push(codigo_de(&LISTAS, &a.lista));
        self.marcado.push(a.marcado);
        self.reiniciar.push(a.reiniciar);
        self.recuo_esq.push(a.recuo_esq);
        self.recuo_dir.push(a.recuo_dir);
        self.primeira.push(a.primeira);
        self.estilo.push(a.estilo);
        self.sem_numero.push(a.sem_numero);
        self.legenda.push(a.legenda);
        self.cor_marcador.push(a.cor_marcador);
        self.apendice.push(a.apendice);
        self.encher.push(a.encher);
        self.mais.push(em_json(&a.mais));
    }

    fn colunas(self) -> Vec<(&'static str, ArrayRef)> {
        vec![
            ("alinhamento", Arc::new(UInt8Array::from(self.alinhamento))),
            ("entrelinha", Arc::new(Float32Array::from(self.entrelinha))),
            ("antes", Arc::new(Float32Array::from(self.antes))),
            ("depois", Arc::new(Float32Array::from(self.depois))),
            ("fundo", Arc::new(UInt32Array::from(self.fundo))),
            ("borda", Arc::new(UInt8Array::from(self.borda))),
            ("cor_borda", Arc::new(UInt32Array::from(self.cor_borda))),
            ("lista", Arc::new(UInt8Array::from(self.lista))),
            ("marcado", Arc::new(BooleanArray::from(self.marcado))),
            ("reiniciar", Arc::new(BooleanArray::from(self.reiniciar))),
            ("recuo_esq", Arc::new(Float32Array::from(self.recuo_esq))),
            ("recuo_dir", Arc::new(Float32Array::from(self.recuo_dir))),
            ("primeira", Arc::new(Float32Array::from(self.primeira))),
            ("estilo", Arc::new(UInt32Array::from(self.estilo))),
            ("sem_numero", Arc::new(BooleanArray::from(self.sem_numero))),
            ("legenda", Arc::new(UInt8Array::from(self.legenda))),
            ("cor_marcador", Arc::new(UInt32Array::from(self.cor_marcador))),
            ("apendice", Arc::new(BooleanArray::from(self.apendice))),
            ("encher", Arc::new(BooleanArray::from(self.encher))),
            ("mais", Arc::new(StringArray::from(self.mais))),
        ]
    }
}

/// A extensão de arquivo de um tipo de imagem.
fn extensao_da_imagem(tipo: &str) -> &'static str {
    match tipo {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        "image/bmp" => "bmp",
        _ => "png",
    }
}

fn tipo_da_imagem(nome: &str) -> &'static str {
    match nome.rsplit('.').next().unwrap_or("") {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        _ => "image/png",
    }
}

impl ColunasDeBloco {
    /// A imagem embutida (`data:`) vai para `assets/` como arquivo; a coluna guarda o caminho.
    fn origem(origem: &str, imagens: &mut Vec<(String, Vec<u8>)>) -> String {
        match base64::de_data_url(origem) {
            Some((tipo, bytes)) => {
                let nome = format!("assets/imagem-{}.{}", imagens.len() + 1, extensao_da_imagem(&tipo));
                imagens.push((nome.clone(), bytes));
                nome
            }
            None => origem.to_string(),
        }
    }

    fn por(&mut self, b: &Bloco, imagens: &mut Vec<(String, Vec<u8>)>) {
        let (mut nivel, mut ordenado, mut fonte, mut linguagem) = (0, false, None, None);
        let (mut largura, mut altura, mut marca_a, mut marca_b, mut lista_sumario) = (0.0, 0.0, false, false, 0);
        match &b.conteudo {
            Conteudo::Titulo { nivel: n, .. } => nivel = *n,
            Conteudo::QuebraDeSecao { inicio } => nivel = *inicio as u8,
            Conteudo::Item { ordenado: o, nivel: n, .. } => (ordenado, nivel) = (*o, *n),
            Conteudo::Codigo { linguagem: l, codigo } => (fonte, linguagem) = (Some(codigo.clone()), Some(l.clone())),
            Conteudo::Tabela { cabecalho, .. } => marca_a = *cabecalho,
            Conteudo::Imagem { origem, largura: w, altura: h, .. } => (fonte, largura, altura) = (Some(Self::origem(origem, imagens)), *w, *h),
            Conteudo::Equacao { latex } => fonte = Some(latex.clone()),
            // O estilo do LaTeX vai no bit de cima do número da lista (a coluna já existe).
            Conteudo::Sumario { niveis, sem_numeros, sem_pontilhado, lista, latex } => (nivel, marca_a, marca_b, lista_sumario) = (*niveis, *sem_numeros, *sem_pontilhado, *lista | if *latex { 0x80 } else { 0 }),
            Conteudo::Bibliografia { todas } => marca_a = *todas,
            _ => {}
        }
        self.id.push(b.id);
        self.tipo.push(tipo_do_bloco(&b.conteudo));
        self.nivel.push(nivel);
        self.ordenado.push(ordenado);
        self.fonte.push(fonte);
        self.linguagem.push(linguagem);
        self.largura.push(largura);
        self.altura.push(altura);
        self.marca_a.push(marca_a);
        self.marca_b.push(marca_b);
        self.lista_sumario.push(lista_sumario);
        self.mesclagens.push(b.mesclagens.iter()
            .map(|m| format!("{},{},{}", m.linha, m.coluna, m.colunas))
            .collect::<Vec<_>>().join(";"));
        self.latex.push(b.latex.clone());
        self.tabela.push(em_json(&b.tabela));
        self.secao.push(b.secao.as_ref().map(|s| serde_json::to_string(s).unwrap_or_default()));
        self.imagem.push(em_json(&b.imagem));
        self.a.por(&b.atrib);
    }
}

fn blocos_arrow(blocos: &[Bloco], imagens: &mut Vec<(String, Vec<u8>)>) -> Result<Vec<u8>, String> {
    let mut c = ColunasDeBloco::default();
    for b in blocos {
        c.por(b, imagens);
    }
    let mut colunas: Vec<(&str, ArrayRef)> = vec![
        ("id", Arc::new(UInt64Array::from(c.id))),
        ("tipo", Arc::new(UInt8Array::from(c.tipo))),
        ("nivel", Arc::new(UInt8Array::from(c.nivel))),
        ("ordenado", Arc::new(BooleanArray::from(c.ordenado))),
        ("fonte", Arc::new(StringArray::from(c.fonte))),
        ("linguagem", Arc::new(StringArray::from(c.linguagem))),
        ("largura", Arc::new(Float32Array::from(c.largura))),
        ("altura", Arc::new(Float32Array::from(c.altura))),
        ("marca_a", Arc::new(BooleanArray::from(c.marca_a))),
        ("marca_b", Arc::new(BooleanArray::from(c.marca_b))),
        ("lista_sumario", Arc::new(UInt8Array::from(c.lista_sumario))),
        ("mesclagens", Arc::new(StringArray::from(c.mesclagens))),
        ("latex", Arc::new(StringArray::from(c.latex))),
        ("tabela", Arc::new(StringArray::from(c.tabela))),
        ("secao", Arc::new(StringArray::from(c.secao))),
        ("imagem", Arc::new(StringArray::from(c.imagem))),
    ];
    colunas.extend(c.a.colunas());
    tabela(colunas, HashMap::new())
}

/// Onde o trecho mora: no texto do bloco, na legenda da imagem ou numa célula de tabela.
const PARTE_TEXTO: u8 = 0;
const PARTE_LEGENDA: u8 = 1;
const PARTE_CELULA: u8 = 2;
/// O texto de uma nota: a linha `dono` é a da chamada (o trecho com a marca).
const PARTE_NOTA: u8 = 3;

/// O tipo do objeto preso ao trecho (coluna `marca`): 0 = nenhum.
fn tipo_da_marca(m: &Marca) -> u8 {
    match m {
        Marca::Nota { .. } => 1,
        Marca::Citacao { .. } => 2,
        Marca::Indice { .. } => 3,
        Marca::Campo { .. } => 4,
        Marca::Comentario { .. } => 5,
        Marca::Imagem { .. } => 6,
        Marca::ObjetoWord { .. } => 7,
        Marca::Controle { .. } => 8,
    }
}

/// Separa as chaves de uma citação dentro da coluna de texto.
const SEPARADOR_DE_CHAVES: char = '\u{1f}';

/// As colunas `marca_a`, `marca_b` e `marca_fim` de cada tipo de objeto.
fn partes_da_marca(m: &Marca) -> (Option<String>, Option<String>, bool) {
    match m {
        Marca::Nota { fim, .. } => (None, None, *fim),
        Marca::Citacao { chaves, pagina } => (Some(chaves.join(&SEPARADOR_DE_CHAVES.to_string())), Some(pagina.clone()), false),
        Marca::Indice { termo, sub } => (Some(termo.clone()), Some(sub.clone()), false),
        Marca::Campo { campo } => (Some(campo.clone()), None, false),
        Marca::Comentario { id, fim } => (Some(id.to_string()), None, *fim),
        Marca::Imagem { origem, largura, altura } => (Some(origem.clone()), Some(format!("{largura} {altura}")), false),
        // O texto da caixa de texto vai em JSON depois do resumo, separado por U+001E.
        Marca::ObjetoWord { xml, resumo, blocos, .. } if !blocos.is_empty() => (Some(xml.clone()), Some(format!("{resumo}\u{1e}{}", serde_json::to_string(blocos).unwrap_or_default())), false),
        Marca::ObjetoWord { xml, resumo, .. } => (Some(xml.clone()), Some(resumo.clone()), false),
        Marca::Controle { xml, fim, bloco } => (Some(xml.clone()), bloco.then(|| "bloco".to_string()), *fim),
    }
}

fn marca_das_partes(tipo: u8, a: String, b: String, fim: bool) -> Option<Marca> {
    Some(match tipo {
        1 => Marca::Nota { fim, texto: Vec::new() },
        2 => Marca::Citacao { chaves: a.split(SEPARADOR_DE_CHAVES).filter(|c| !c.is_empty()).map(str::to_string).collect(), pagina: b },
        3 => Marca::Indice { termo: a, sub: b },
        4 => Marca::Campo { campo: a },
        5 => Marca::Comentario { id: a.parse().unwrap_or(0), fim },
        6 => {
            let (w, h) = b.split_once(' ').unwrap_or(("0", "0"));
            Marca::Imagem { origem: a, largura: w.parse().unwrap_or(0.0), altura: h.parse().unwrap_or(0.0) }
        }
        7 => {
            let (resumo, blocos) = b.split_once('\u{1e}').map_or((b.as_str(), Vec::new()), |(r, j)| (r, serde_json::from_str(j).unwrap_or_default()));
            Marca::ObjetoWord { xml: a, partes: Vec::new(), resumo: resumo.to_string(), blocos }
        }
        8 => Marca::Controle { xml: a, fim, bloco: b == "bloco" },
        _ => return None,
    })
}

#[derive(Default)]
struct ColunasDeTrecho {
    bloco: Vec<u32>,
    parte: Vec<u8>,
    linha: Vec<u32>,
    coluna: Vec<u32>,
    texto: Vec<String>,
    estilo: Vec<u8>,
    link: Vec<Option<String>>,
    tamanho: Vec<f32>,
    cor: Vec<u32>,
    realce: Vec<u32>,
    posicao: Vec<i8>,
    /// O nome da fonte e o estilo de caractere (colunas opcionais: faltam nos arquivos de antes delas).
    familia: Vec<Option<&'static str>>,
    estilo_car: Vec<Option<u32>>,
    /// O resto do `w:rPr` (coluna opcional, da fase 2 do .docx).
    mais: Vec<Option<String>>,
    marca: Vec<u8>,
    marca_a: Vec<Option<String>>,
    marca_b: Vec<Option<String>>,
    marca_fim: Vec<bool>,
    dono: Vec<Option<u32>>,
    /// Sugestão: 0 = nenhuma, 1 = inserção, 2 = remoção; e quem, e quando.
    rev: Vec<u8>,
    rev_autor: Vec<Option<String>>,
    rev_quando: Vec<Option<String>>,
    /// As partes dos objetos do Word: `(linha do trecho, parte)`, para `data/sacola.arrow`.
    sacola: Vec<(u32, ParteWord)>,
}

impl ColunasDeTrecho {
    fn por(&mut self, bloco: usize, parte: u8, (linha, coluna): (u32, u32), ts: &[Trecho]) {
        self.por_com_dono(bloco, parte, (linha, coluna), ts, None);
    }

    fn por_com_dono(&mut self, bloco: usize, parte: u8, (linha, coluna): (u32, u32), ts: &[Trecho], dono: Option<u32>) {
        for t in ts {
            let aqui = self.texto.len() as u32;
            self.bloco.push(bloco as u32);
            self.parte.push(parte);
            self.linha.push(linha);
            self.coluna.push(coluna);
            self.texto.push(t.texto.clone());
            self.estilo.push(t.estilo);
            self.link.push(t.link.clone());
            self.tamanho.push(t.fmt.tamanho);
            self.cor.push(t.fmt.cor);
            self.realce.push(t.fmt.realce);
            self.posicao.push(t.fmt.posicao);
            self.familia.push((!t.fmt.familia.vazia()).then(|| t.fmt.familia.nome()));
            self.estilo_car.push((t.fmt.estilo_de_caractere != 0).then_some(t.fmt.estilo_de_caractere));
            self.mais.push(em_json(&t.fmt.mais));
            self.dono.push(dono);
            let r = t.rev.as_deref();
            // 1 e 2 = inserção e remoção; 3 e 4 = as mesmas, de texto movido.
            self.rev.push(r.map_or(0, |r| if r.tipo == TipoDeRevisao::Insercao { 1 } else { 2 } + if r.movido { 2 } else { 0 }));
            self.rev_autor.push(r.map(|r| r.autor.clone()));
            self.rev_quando.push(r.map(|r| r.quando.clone()));
            let Some(m) = t.marca.as_deref() else {
                self.marca.push(0);
                self.marca_a.push(None);
                self.marca_b.push(None);
                self.marca_fim.push(false);
                continue;
            };
            if let Marca::ObjetoWord { partes, .. } = m {
                self.sacola.extend(partes.iter().map(|p| (aqui, p.clone())));
            }
            let (a, b, fim) = partes_da_marca(m);
            self.marca.push(tipo_da_marca(m));
            self.marca_a.push(a);
            self.marca_b.push(b);
            self.marca_fim.push(fim);
            if let Marca::Nota { texto, .. } = m {
                self.por_com_dono(bloco, PARTE_NOTA, (linha, coluna), texto, Some(aqui));
            }
        }
    }
}

/// `data/trechos.arrow` e `data/sacola.arrow`.
fn trechos_arrow(blocos: &[Bloco]) -> Result<(Vec<u8>, Vec<u8>), String> {
    let mut c = ColunasDeTrecho::default();
    for (i, b) in blocos.iter().enumerate() {
        match &b.conteudo {
            Conteudo::Titulo { texto, .. } | Conteudo::Paragrafo { texto } | Conteudo::Item { texto, .. } | Conteudo::Citacao { texto } => c.por(i, PARTE_TEXTO, (0, 0), texto),
            Conteudo::Imagem { legenda, .. } => c.por(i, PARTE_LEGENDA, (0, 0), legenda),
            Conteudo::Tabela { linhas, .. } => {
                for (l, linha) in linhas.iter().enumerate() {
                    for (k, celula) in linha.iter().enumerate() {
                        c.por(i, PARTE_CELULA, (l as u32, k as u32), celula);
                    }
                }
            }
            _ => {}
        }
    }
    let sacola = sacola_arrow(&std::mem::take(&mut c.sacola))?;
    let trechos = tabela(
        vec![
            ("bloco", Arc::new(UInt32Array::from(c.bloco))),
            ("parte", Arc::new(UInt8Array::from(c.parte))),
            ("linha", Arc::new(UInt32Array::from(c.linha))),
            ("coluna", Arc::new(UInt32Array::from(c.coluna))),
            ("texto", Arc::new(StringArray::from(c.texto))),
            ("estilo", Arc::new(UInt8Array::from(c.estilo))),
            ("link", Arc::new(StringArray::from(c.link))),
            ("tamanho", Arc::new(Float32Array::from(c.tamanho))),
            ("cor", Arc::new(UInt32Array::from(c.cor))),
            ("realce", Arc::new(UInt32Array::from(c.realce))),
            ("posicao", Arc::new(Int8Array::from(c.posicao))),
            ("familia", Arc::new(StringArray::from(c.familia))),
            ("estilo_car", Arc::new(UInt32Array::from(c.estilo_car))),
            ("mais", Arc::new(StringArray::from(c.mais))),
            ("marca", Arc::new(UInt8Array::from(c.marca))),
            ("marca_a", Arc::new(StringArray::from(c.marca_a))),
            ("marca_b", Arc::new(StringArray::from(c.marca_b))),
            ("marca_fim", Arc::new(BooleanArray::from(c.marca_fim))),
            ("dono", Arc::new(UInt32Array::from(c.dono))),
            ("rev", Arc::new(UInt8Array::from(c.rev))),
            ("rev_autor", Arc::new(StringArray::from(c.rev_autor))),
            ("rev_quando", Arc::new(StringArray::from(c.rev_quando))),
        ],
        HashMap::new(),
    )?;
    Ok((trechos, sacola))
}

fn sacola_arrow(s: &[(u32, ParteWord)]) -> Result<Vec<u8>, String> {
    let textos = |f: fn(&ParteWord) -> &str| -> ArrayRef { Arc::new(StringArray::from(s.iter().map(|(_, p)| f(p)).collect::<Vec<_>>())) };
    tabela(
        vec![
            ("trecho", Arc::new(UInt32Array::from(s.iter().map(|(t, _)| *t).collect::<Vec<_>>()))),
            ("rid", textos(|p| &p.rid)),
            ("tipo", textos(|p| &p.tipo)),
            ("caminho", textos(|p| &p.caminho)),
            ("externo", Arc::new(BooleanArray::from(s.iter().map(|(_, p)| p.externo).collect::<Vec<_>>()))),
            ("conteudo", textos(|p| &p.conteudo)),
            ("bytes", Arc::new(BinaryArray::from(s.iter().map(|(_, p)| p.bytes.as_slice()).collect::<Vec<_>>()))),
        ],
        HashMap::new(),
    )
}

/// As partes de `data/sacola.arrow` de volta nos objetos do Word (pela linha do trecho).
fn ler_sacola(t: &Lida, lidos: &mut [Trecho]) -> Result<(), String> {
    let trecho = t.col::<UInt32Array>("trecho")?;
    let (rid, tipo, caminho, conteudo) = (t.col::<StringArray>("rid")?, t.col::<StringArray>("tipo")?, t.col::<StringArray>("caminho")?, t.col::<StringArray>("conteudo")?);
    let (externo, bytes) = (t.col::<BooleanArray>("externo")?, t.col::<BinaryArray>("bytes")?);
    for i in 0..t.linhas() {
        let Some(Marca::ObjetoWord { partes, .. }) = lidos.get_mut(trecho.value(i) as usize).and_then(|t| t.marca.as_deref_mut()) else {
            return Err("sacola.arrow aponta para um trecho que não é objeto do Word.".into());
        };
        partes.push(ParteWord { rid: rid.value(i).into(), tipo: tipo.value(i).into(), caminho: caminho.value(i).into(), externo: externo.value(i), conteudo: conteudo.value(i).into(), bytes: bytes.value(i).to_vec() });
    }
    Ok(())
}

/// A forma de cada tabela: uma linha por linha da tabela, com quantas células ela tem
/// (célula vazia não tem trecho, então a forma não sai dos trechos).
fn tabelas_arrow(blocos: &[Bloco]) -> Result<Vec<u8>, String> {
    let (mut bloco, mut linha, mut celulas) = (Vec::new(), Vec::new(), Vec::new());
    for (i, b) in blocos.iter().enumerate() {
        if let Conteudo::Tabela { linhas, .. } = &b.conteudo {
            for (l, cels) in linhas.iter().enumerate() {
                bloco.push(i as u32);
                linha.push(l as u32);
                celulas.push(cels.len() as u32);
            }
        }
    }
    tabela(
        vec![("bloco", Arc::new(UInt32Array::from(bloco))), ("linha", Arc::new(UInt32Array::from(linha))), ("celulas", Arc::new(UInt32Array::from(celulas)))],
        HashMap::new(),
    )
}

fn estilos_arrow(e: &Estilos) -> Result<Vec<u8>, String> {
    let padrao = (!e.padrao.eh_padrao()).then(|| Estilo { id: 0, ..e.padrao.clone() });
    let l: Vec<&Estilo> = padrao.iter().chain(&e.lista).collect();
    let u32s = |f: fn(&Estilo) -> u32| -> ArrayRef { Arc::new(UInt32Array::from(l.iter().map(|x| f(x)).collect::<Vec<_>>())) };
    let f32s = |f: fn(&Estilo) -> f32| -> ArrayRef { Arc::new(Float32Array::from(l.iter().map(|x| f(x)).collect::<Vec<_>>())) };
    let mut meta = HashMap::new();
    if !e.tema.is_empty() {
        meta.insert("tema".to_string(), e.tema.clone());
    }
    if !e.de_tabela.is_empty() {
        meta.insert("estilos_de_tabela".to_string(), e.de_tabela.clone());
    }
    if !e.legendas.is_empty() {
        meta.insert("legendas".to_string(), e.legendas.join("\u{1f}"));
    }
    tabela(
        vec![
            ("id", u32s(|x| x.id)),
            ("nome", Arc::new(StringArray::from(l.iter().map(|x| x.nome.as_str()).collect::<Vec<_>>()))),
            ("base", Arc::new(UInt8Array::from(l.iter().map(|x| x.base).collect::<Vec<_>>()))),
            ("bits", Arc::new(UInt8Array::from(l.iter().map(|x| x.bits).collect::<Vec<_>>()))),
            ("tamanho", f32s(|x| x.tamanho)),
            ("cor", u32s(|x| x.cor)),
            ("alinhamento", Arc::new(UInt8Array::from(l.iter().map(|x| codigo_de(&ALINHAMENTOS, &x.alinhamento)).collect::<Vec<_>>()))),
            ("antes", Arc::new(Float32Array::from(l.iter().map(|x| x.antes).collect::<Vec<_>>()))),
            ("depois", Arc::new(Float32Array::from(l.iter().map(|x| x.depois).collect::<Vec<_>>()))),
            ("entrelinha", f32s(|x| x.entrelinha)),
            // Da fase 1 do .docx (opcionais na leitura).
            ("codigo", Arc::new(StringArray::from(l.iter().map(|x| x.codigo.as_str()).collect::<Vec<_>>()))),
            ("caractere", Arc::new(BooleanArray::from(l.iter().map(|x| x.caractere).collect::<Vec<_>>()))),
            ("herda", u32s(|x| x.herda)),
            ("proximo", u32s(|x| x.proximo)),
            ("familia", Arc::new(StringArray::from(l.iter().map(|x| x.familia.nome()).collect::<Vec<_>>()))),
            ("realce", u32s(|x| x.realce)),
            ("posicao", Arc::new(Int8Array::from(l.iter().map(|x| x.posicao).collect::<Vec<_>>()))),
            ("fundo", u32s(|x| x.fundo)),
            ("recuo_esq", f32s(|x| x.recuo_esq)),
            ("recuo_dir", f32s(|x| x.recuo_dir)),
            ("primeira", f32s(|x| x.primeira)),
            // Da fase 2 (opcionais na leitura).
            ("mais_letra", Arc::new(StringArray::from(l.iter().map(|x| em_json(&x.mais_letra)).collect::<Vec<_>>()))),
            ("mais_paragrafo", Arc::new(StringArray::from(l.iter().map(|x| em_json(&x.mais_paragrafo)).collect::<Vec<_>>()))),
            ("desligados", u32s(|x| x.desligados)),
        ],
        meta,
    )
}

/// As listas do Word, um nível por linha; `lista` é o índice em `Estilos::listas`.
fn listas_arrow(l: &[ListaDoWord]) -> Result<Vec<u8>, String> {
    let n: Vec<(u32, &ListaDoWord, &NivelDeLista)> = l.iter().enumerate().flat_map(|(i, x)| x.niveis.iter().map(move |n| (i as u32, x, n))).collect();
    let s = |f: for<'a> fn(&'a ListaDoWord, &'a NivelDeLista) -> &'a str| -> ArrayRef { Arc::new(StringArray::from(n.iter().map(|(_, x, v)| f(x, v)).collect::<Vec<_>>())) };
    tabela(
        vec![
            ("lista", Arc::new(UInt32Array::from(n.iter().map(|x| x.0).collect::<Vec<_>>()))),
            ("estilo", s(|x, _| &x.estilo)),
            ("formato", s(|_, v| &v.formato)),
            ("texto", s(|_, v| &v.texto)),
            ("fonte", s(|_, v| &v.fonte)),
            ("inicio", Arc::new(UInt32Array::from(n.iter().map(|x| x.2.inicio).collect::<Vec<_>>()))),
            ("recuo", Arc::new(Float32Array::from(n.iter().map(|x| x.2.recuo).collect::<Vec<_>>()))),
            ("pendurado", Arc::new(Float32Array::from(n.iter().map(|x| x.2.pendurado).collect::<Vec<_>>()))),
        ],
        HashMap::new(),
    )
}

fn ler_listas(t: &Lida) -> Result<Vec<ListaDoWord>, String> {
    let (lista, estilo, formato, texto, fonte) = (t.col::<UInt32Array>("lista")?, t.col::<StringArray>("estilo")?, t.col::<StringArray>("formato")?, t.col::<StringArray>("texto")?, t.col::<StringArray>("fonte")?);
    let (inicio, recuo, pendurado) = (t.col::<UInt32Array>("inicio")?, t.col::<Float32Array>("recuo")?, t.col::<Float32Array>("pendurado")?);
    let mut l: Vec<ListaDoWord> = Vec::new();
    for i in 0..t.linhas() {
        let k = lista.value(i) as usize;
        if k > l.len() || k > u16::MAX as usize {
            return Err("listas.arrow fora de ordem.".into());
        }
        if k == l.len() {
            l.push(ListaDoWord { niveis: Vec::new(), estilo: estilo.value(i).to_string() });
        }
        l[k].niveis.push(NivelDeLista { formato: formato.value(i).into(), texto: texto.value(i).into(), inicio: inicio.value(i), recuo: recuo.value(i), pendurado: pendurado.value(i), fonte: fonte.value(i).into() });
    }
    Ok(l)
}

fn marcadores_arrow(m: &[Marcador]) -> Result<Vec<u8>, String> {
    tabela(
        vec![
            ("nome", Arc::new(StringArray::from(m.iter().map(|x| x.nome.as_str()).collect::<Vec<_>>()))),
            ("bloco", Arc::new(UInt64Array::from(m.iter().map(|x| x.bloco).collect::<Vec<_>>()))),
        ],
        HashMap::new(),
    )
}

// ---------------------------------------------------------------------------
// Ler
// ---------------------------------------------------------------------------

fn ler_parte(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, nome: &str) -> Result<Vec<u8>, String> {
    let f = zip.by_name(nome).map_err(|_| format!("O .fdoz está incompleto: falta \"{nome}\"."))?;
    crate::ler_limitado(f).map_err(|e| format!("Não consegui ler \"{nome}\" do .fdoz: {e}"))
}

/// Lê um `.fdoz`.
pub fn ler(bytes: &[u8]) -> Result<Documento, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("Isto não é um .fdoz válido: {e}"))?;
    if zip.by_name("manifest.json").is_ok() {
        return Err("Este .fdoz é do formato antigo (JSON, da fase de testes), que não abre mais.".into());
    }
    let mut parte = |nome: &'static str| ler_parte(&mut zip, &format!("data/{nome}.arrow")).and_then(|b| Lida::de(&b, nome));
    let documento = parte("documento")?;
    let formato: u32 = documento.meta("formato").and_then(|v| v.parse().ok()).unwrap_or(0);
    if formato > VERSAO {
        return Err(format!("Este .fdoz é da versão {formato} do formato; este FeatherDocs entende até a {VERSAO}. Atualize o aplicativo."));
    }
    let (blocos, trechos, tabelas, estilos, marcadores) = (parte("blocos")?, parte("trechos")?, parte("tabelas")?, parte("estilos")?, parte("marcadores")?);
    let mut doc = ler_documento(&documento)?;
    ler_estilos(&estilos, &mut doc.estilos)?;
    doc.marcadores = ler_marcadores(&marcadores)?;
    if let Ok(b) = ler_parte(&mut zip, "data/listas.arrow") {
        doc.estilos.listas = ler_listas(&Lida::de(&b, "listas")?)?;
    }
    if let Ok(b) = ler_parte(&mut zip, "data/fontes.arrow") {
        doc.bibliografia.fontes = ler_fontes(&Lida::de(&b, "fontes")?)?;
    }
    if let Ok(b) = ler_parte(&mut zip, "data/comentarios.arrow") {
        let respostas = ler_parte(&mut zip, "data/respostas.arrow").ok().map(|r| Lida::de(&r, "respostas")).transpose()?;
        doc.comentarios = ler_comentarios(&Lida::de(&b, "comentarios")?, respostas.as_ref())?;
    }
    doc.blocos = ler_blocos(&blocos)?;
    let sacola = ler_parte(&mut zip, "data/sacola.arrow").ok().map(|b| Lida::de(&b, "sacola")).transpose()?;
    encher_trechos(&mut doc.blocos, &trechos, &tabelas, sacola.as_ref())?;
    // A forma da tabela só fica disponível após encher_trechos. Recusar
    // arquivos hostis que esconderiam texto, ultrapassariam limites ou
    // conteriam mesclagens sobrepostas antes de iniciar a composição.
    for bloco in &doc.blocos {
        if bloco.mesclagens.is_empty() {
            continue;
        }
        let Conteudo::Tabela { linhas, .. } = &bloco.conteudo else {
            return Err("Mesclagens só são válidas em tabelas.".into());
        };
        let colunas = linhas.first().map_or(0, Vec::len);
        let mut ocupadas = std::collections::HashSet::new();
        if linhas.len() > 200 || colunas > 30
            || linhas.iter().any(|l| l.len() != colunas)
            || bloco.mesclagens.len() > 3000 {
            return Err("Mesclagens inválidas em tabela FDOZ.".into());
        }
        for m in &bloco.mesclagens {
            if m.colunas < 2 || m.linha >= linhas.len()
                || m.coluna.checked_add(m.colunas).map_or(true, |fim| fim > colunas) {
                return Err("Mesclagem fora dos limites em FDOZ.".into());
            }
            for coluna in m.coluna..m.coluna + m.colunas {
                if !ocupadas.insert((m.linha, coluna))
                    || (coluna > m.coluna && !linhas[m.linha][coluna].is_empty()) {
                    return Err("Mesclagem sobreposta ou destrutiva em FDOZ.".into());
                }
            }
        }
    }
    for b in &mut doc.blocos {
        if let Conteudo::Imagem { origem, .. } = &mut b.conteudo {
            if origem.starts_with("assets/") {
                let dados = ler_parte(&mut zip, origem)?;
                *origem = base64::para_data_url(tipo_da_imagem(origem), &dados);
            }
        }
    }
    doc.acertar_ids();
    Ok(doc)
}

/// O fio e a letra da faixa `k` (0 cabeçalho, 1 rodapé) da coluna `faixas_fmt` ("fio estilo tamanho cor;…"),
/// que o `.fdoz` anterior a 18/10/2026 não tem.
/// A imagem `k` da coluna `faixas_imagens` ("largura altura origem" por linha).
fn com_imagem(mut f: Faixas, imagens: &str, k: usize) -> Faixas {
    if let Some(linha) = imagens.lines().nth(k) {
        let mut partes = linha.splitn(3, ' ');
        f.imagem_largura = partes.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        f.imagem_altura = partes.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        f.imagem = partes.next().unwrap_or("").to_string();
    }
    f
}

fn com_fmt(mut f: Faixas, fmt: &str, k: usize) -> Faixas {
    if let Some([fio, estilo, tamanho, cor]) = fmt.split(';').nth(k).map(|p| p.split(' ').collect::<Vec<_>>()).and_then(|v| <[&str; 4]>::try_from(v).ok()) {
        f.fio = fio.parse().unwrap_or(0.0);
        f.estilo = estilo.parse().unwrap_or(0);
        f.tamanho = tamanho.parse().unwrap_or(0.0);
        f.cor = u32::from_str_radix(cor, 16).unwrap_or(0);
    }
    f
}

fn ler_documento(t: &Lida) -> Result<Documento, String> {
    if t.linhas() != 1 {
        return Err("documento.arrow tem que ter uma linha.".into());
    }
    let s = |c: &str| t.col::<StringArray>(c).map(|a| a.value(0).to_string());
    let f = |c: &str| t.col::<Float32Array>(c).map(|a| a.value(0));
    let u = |c: &str| t.col::<UInt32Array>(c).map(|a| a.value(0));
    let b8 = |c: &str| t.col::<UInt8Array>(c).map(|a| a.value(0));
    let pagina = ConfigPagina { largura: f("largura")?, altura: f("altura")?, margem_sup: f("margem_sup")?, margem_dir: f("margem_dir")?, margem_inf: f("margem_inf")?, margem_esq: f("margem_esq")? };
    let (fmt, imagens) = (s("faixas_fmt").unwrap_or_default(), s("faixas_imagens").unwrap_or_default());
    // As colunas da primeira página vieram depois: faltando, ficam vazias.
    let s0 = |c: &str| s(c).unwrap_or_default();
    let faixa = |esq: String, centro: String, dir: String, k: usize| com_imagem(com_fmt(Faixas { esquerda: esq, centro, direita: dir, ..Faixas::default() }, &fmt, k), &imagens, k);
    let extras = Extras {
        cabecalho: faixa(s("cabecalho_esq")?, s("cabecalho_centro")?, s("cabecalho_dir")?, 0),
        rodape: faixa(s("rodape_esq")?, s("rodape_centro")?, s("rodape_dir")?, 1),
        cabecalho_primeira: faixa(s0("cabecalho1_esq"), s0("cabecalho1_centro"), s0("cabecalho1_dir"), 2),
        rodape_primeira: faixa(s0("rodape1_esq"), s0("rodape1_centro"), s0("rodape1_dir"), 3),
        primeira_diferente: t.col::<BooleanArray>("primeira_diferente")?.value(0),
        cor_pagina: u("cor_pagina")?,
        marca: s("marca")?,
        marca_cor: u("marca_cor")?,
        borda_cor: u("borda_cor")?,
        borda_espessura: f("borda_espessura")?,
        borda_distancia: f("borda_distancia")?,
        vertical: valor_de(&VERTICAIS, b8("vertical")?),
        numerar_linhas: valor_de(&LINHAS_NUMERADAS, b8("numerar_linhas")?),
        // As colunas de depois da primeira versão do formato 2 podem faltar.
        formato_notas: b8("formato_notas").unwrap_or(0),
        formato_notas_fim: b8("formato_notas_fim").unwrap_or(2),
        controlar_alteracoes: t.col::<BooleanArray>("controlar_alteracoes").is_ok_and(|a| a.value(0)),
        protegido: t.col::<BooleanArray>("protegido").is_ok_and(|a| a.value(0)),
        composicao_tex: t.col::<BooleanArray>("composicao_tex").is_ok_and(|a| a.value(0)),
        pagina_inicial: u("pagina_inicial").unwrap_or(1),
        tinta: Some(s0("tinta")).filter(|t| !t.is_empty())
            .map(|t| t.split('\u{1e}').map(str::to_string).collect()).unwrap_or_default(),
        decoracoes: serde_json::from_str(&s0("decoracoes")).unwrap_or_default(),
        documento: serde_json::from_str(&s0("documento")).unwrap_or_default(),
    };
    let mut doc = Documento::vazio();
    doc.meta = Meta { titulo: s("titulo")?, autor: s("autor")? };
    doc.pagina = pagina;
    doc.extras = extras;
    let blocos: [(Vec<Bloco>, String); 4] = de_json(t, "faixas_blocos", 0);
    let x = &mut doc.extras;
    for (f, (b, lida)) in [&mut x.cabecalho, &mut x.rodape, &mut x.cabecalho_primeira, &mut x.rodape_primeira].into_iter().zip(blocos) {
        (f.blocos, f.lida) = (b, lida);
    }
    doc.estilos.numeracao = valor_de(&NUMERACOES, b8("numeracao")?);
    doc.bibliografia.estilo = valor_de(&ESTILOS_DE_CITACAO, b8("estilo_citacao").unwrap_or(0));
    doc.bibliografia.sem_sl_sn = t.col::<BooleanArray>("sem_sl_sn").is_ok_and(|a| a.value(0));
    doc.proximo_id = t.col::<UInt64Array>("proximo_id")?.value(0);
    Ok(doc)
}

fn ler_atrib(t: &Lida, i: usize) -> Result<Atrib, String> {
    let f = |c: &str| t.col::<Float32Array>(c).map(|a| a.value(i));
    let fo = |c: &str| t.col::<Float32Array>(c).map(|a| opcional(a, i, Float32Array::value));
    let u = |c: &str| t.col::<UInt32Array>(c).map(|a| a.value(i));
    let b8 = |c: &str| t.col::<UInt8Array>(c).map(|a| a.value(i));
    let b = |c: &str| t.col::<BooleanArray>(c).map(|a| a.value(i));
    Ok(Atrib {
        alinhamento: valor_de(&ALINHAMENTOS, b8("alinhamento")?),
        entrelinha: f("entrelinha")?,
        antes: fo("antes")?,
        depois: fo("depois")?,
        fundo: u("fundo")?,
        borda: b8("borda")?,
        cor_borda: u("cor_borda")?,
        lista: valor_de(&LISTAS, b8("lista")?),
        marcado: b("marcado")?,
        reiniciar: b("reiniciar")?,
        recuo_esq: f("recuo_esq")?,
        recuo_dir: f("recuo_dir")?,
        primeira: f("primeira")?,
        estilo: u("estilo")?,
        sem_numero: b("sem_numero")?,
        legenda: b8("legenda")?,
        cor_marcador: u("cor_marcador")?,
        // Coluna nova (18/10/2026): o .fdoz de antes não tem.
        apendice: b("apendice").unwrap_or(false),
        encher: b("encher").unwrap_or(false),
        mais: de_json(t, "mais", i),
    })
}

/// Os blocos com o conteúdo próprio; o texto (trechos) entra depois, em `encher_trechos`.
fn ler_blocos(t: &Lida) -> Result<Vec<Bloco>, String> {
    let (id, tipo, nivel, ordenado) = (t.col::<UInt64Array>("id")?, t.col::<UInt8Array>("tipo")?, t.col::<UInt8Array>("nivel")?, t.col::<BooleanArray>("ordenado")?);
    let (fonte, linguagem) = (t.col::<StringArray>("fonte")?, t.col::<StringArray>("linguagem")?);
    let (largura, altura) = (t.col::<Float32Array>("largura")?, t.col::<Float32Array>("altura")?);
    let (marca_a, marca_b, lista) = (t.col::<BooleanArray>("marca_a")?, t.col::<BooleanArray>("marca_b")?, t.col::<UInt8Array>("lista_sumario")?);
    (0..t.linhas())
        .map(|i| {
            let texto_de = |a: &StringArray| opcional(a, i, |a, i| a.value(i).to_string()).unwrap_or_default();
            let conteudo = match tipo.value(i) {
                0 => Conteudo::Titulo { nivel: nivel.value(i), texto: Vec::new() },
                1 => Conteudo::Paragrafo { texto: Vec::new() },
                2 => Conteudo::Item { ordenado: ordenado.value(i), nivel: nivel.value(i), texto: Vec::new() },
                3 => Conteudo::Citacao { texto: Vec::new() },
                4 => Conteudo::Codigo { linguagem: texto_de(linguagem), codigo: texto_de(fonte) },
                5 => Conteudo::Tabela { cabecalho: marca_a.value(i), linhas: Vec::new() },
                6 => Conteudo::Imagem { origem: texto_de(fonte), legenda: Vec::new(), largura: largura.value(i), altura: altura.value(i) },
                7 => Conteudo::Equacao { latex: texto_de(fonte) },
                8 => Conteudo::Sumario { niveis: nivel.value(i), sem_numeros: marca_a.value(i), sem_pontilhado: marca_b.value(i), lista: lista.value(i) & 0x7F, latex: lista.value(i) & 0x80 != 0 },
                9 => Conteudo::Regua,
                10 => Conteudo::QuebraDePagina,
                11 => Conteudo::Bibliografia { todas: marca_a.value(i) },
                12 => Conteudo::IndiceRemissivo,
                13 => Conteudo::QuebraDeColuna,
                14 => Conteudo::QuebraDeSecao { inicio: [InicioDeSecao::ProximaPagina, InicioDeSecao::Continua, InicioDeSecao::PaginaPar, InicioDeSecao::PaginaImpar][usize::from(nivel.value(i).min(3))] },
                outro => return Err(format!("blocos.arrow: tipo de bloco desconhecido ({outro}).")),
            };
            // A coluna é opcional para manter leitura de .fdoz antigos.
            let codificado = t.col::<StringArray>("mesclagens").ok()
                .map(|v| v.value(i)).unwrap_or("");
            let mut mesclagens = Vec::new();
            if !codificado.is_empty() {
                for item in codificado.split(';') {
                    let numeros: Vec<usize> = item.split(',').map(str::parse::<usize>)
                        .collect::<Result<_, _>>()
                        .map_err(|_| "Mesclagem inválida em blocos.arrow.")?;
                    if numeros.len() != 3 || numeros[2] < 2 {
                        return Err("Mesclagem inválida em blocos.arrow.".into());
                    }
                    mesclagens.push(MesclagemTabela {
                        linha: numeros[0], coluna: numeros[1], colunas: numeros[2],
                    });
                }
            }
            // Opcional também: .fdoz antigos não têm.
            let latex = t.col::<StringArray>("latex").ok().map_or(String::new(), |v| v.value(i).to_string());
            Ok(Bloco { id: id.value(i), conteudo, atrib: ler_atrib(t, i)?, mesclagens, latex, tabela: de_json(t, "tabela", i), secao: de_json(t, "secao", i), imagem: de_json(t, "imagem", i) })
        })
        .collect()
}

/// Põe os trechos nos blocos (no texto, na legenda ou na célula) e dá às tabelas a forma delas.
fn encher_trechos(blocos: &mut [Bloco], trechos: &Lida, tabelas: &Lida, sacola: Option<&Lida>) -> Result<(), String> {
    let (tb, tl, tc) = (tabelas.col::<UInt32Array>("bloco")?, tabelas.col::<UInt32Array>("linha")?, tabelas.col::<UInt32Array>("celulas")?);
    for i in 0..tabelas.linhas() {
        if let Some(Conteudo::Tabela { linhas, .. }) = blocos.get_mut(tb.value(i) as usize).map(|b| &mut b.conteudo) {
            let l = tl.value(i) as usize;
            if linhas.len() <= l {
                linhas.resize(l + 1, Vec::new());
            }
            linhas[l] = vec![Celula::new(); tc.value(i) as usize];
        }
    }
    let (bloco, parte, linha, coluna) = (trechos.col::<UInt32Array>("bloco")?, trechos.col::<UInt8Array>("parte")?, trechos.col::<UInt32Array>("linha")?, trechos.col::<UInt32Array>("coluna")?);
    let (texto, estilo, link) = (trechos.col::<StringArray>("texto")?, trechos.col::<UInt8Array>("estilo")?, trechos.col::<StringArray>("link")?);
    let (tamanho, cor, realce, posicao) = (trechos.col::<Float32Array>("tamanho")?, trechos.col::<UInt32Array>("cor")?, trechos.col::<UInt32Array>("realce")?, trechos.col::<Int8Array>("posicao")?);
    let (familia, estilo_car) = (trechos.col::<StringArray>("familia").ok(), trechos.col::<UInt32Array>("estilo_car").ok());
    let mut lidos: Vec<Trecho> = (0..trechos.linhas())
        .map(|i| Trecho {
            texto: texto.value(i).to_string(),
            estilo: estilo.value(i),
            link: opcional(link, i, |a, i| a.value(i).to_string()),
            fmt: Fmt {
                tamanho: tamanho.value(i),
                cor: cor.value(i),
                realce: realce.value(i),
                posicao: posicao.value(i),
                familia: familia.and_then(|a| opcional(a, i, |a, i| Familia::de(a.value(i)))).unwrap_or_default(),
                estilo_de_caractere: estilo_car.and_then(|a| opcional(a, i, UInt32Array::value)).unwrap_or(0),
                mais: de_json(trechos, "mais", i),
            },
            marca: marca_lida(trechos, i).map(Box::new),
            rev: rev_lida(trechos, i).map(Box::new),
            ..Trecho::default()
        })
        .collect();
    if let Some(s) = sacola {
        ler_sacola(s, &mut lidos)?;
    }
    // O texto das notas vai para dentro da chamada (a linha `dono`), de trás para a frente: as linhas
    // de uma nota vêm logo depois da chamada dela.
    let donos: Vec<Option<u32>> = (0..trechos.linhas()).map(|i| trechos.lote.column_by_name("dono").and_then(|c| c.as_any().downcast_ref::<UInt32Array>()).and_then(|a| opcional(a, i, UInt32Array::value))).collect();
    let mut da_nota: Vec<Vec<Trecho>> = vec![Vec::new(); lidos.len()];
    for i in (0..lidos.len()).rev() {
        if let Some(d) = donos[i] {
            let mut t = std::mem::take(&mut lidos[i]);
            if let Some(Marca::Nota { texto, .. }) = t.marca.as_deref_mut() {
                texto.append(&mut std::mem::take(&mut da_nota[i]));
            }
            da_nota.get_mut(d as usize).ok_or("trechos.arrow: nota sem chamada.")?.insert(0, t);
        }
    }
    for (i, mut t) in lidos.into_iter().enumerate() {
        if donos[i].is_some() {
            continue;
        }
        if let Some(Marca::Nota { texto, .. }) = t.marca.as_deref_mut() {
            *texto = std::mem::take(&mut da_nota[i]);
        }
        let Some(b) = blocos.get_mut(bloco.value(i) as usize) else { return Err("trechos.arrow aponta para um bloco que não existe.".into()) };
        let destino: Option<&mut Vec<Trecho>> = match (&mut b.conteudo, parte.value(i)) {
            (Conteudo::Titulo { texto, .. } | Conteudo::Paragrafo { texto } | Conteudo::Item { texto, .. } | Conteudo::Citacao { texto }, PARTE_TEXTO) => Some(texto),
            (Conteudo::Imagem { legenda, .. }, PARTE_LEGENDA) => Some(legenda),
            (Conteudo::Tabela { linhas, .. }, PARTE_CELULA) => linhas.get_mut(linha.value(i) as usize).and_then(|l| l.get_mut(coluna.value(i) as usize)),
            _ => None,
        };
        destino.ok_or("trechos.arrow não bate com os blocos.")?.push(t);
    }
    Ok(())
}

/// A sugestão do trecho `i` (as colunas `rev…` faltam nos arquivos de antes delas).
fn rev_lida(t: &Lida, i: usize) -> Option<Revisao> {
    let tipo = match t.col::<UInt8Array>("rev").ok()?.value(i) {
        1 | 3 => TipoDeRevisao::Insercao,
        2 | 4 => TipoDeRevisao::Remocao,
        _ => return None,
    };
    let texto = |c: &str| t.col::<StringArray>(c).ok().and_then(|a| opcional(a, i, |a, i| a.value(i).to_string())).unwrap_or_default();
    let movido = matches!(t.col::<UInt8Array>("rev").ok()?.value(i), 3 | 4);
    Some(Revisao { tipo, autor: texto("rev_autor"), quando: texto("rev_quando"), movido })
}

fn comentarios_arrow(c: &[Comentario]) -> Result<Vec<u8>, String> {
    tabela(
        vec![
            ("id", Arc::new(UInt64Array::from(c.iter().map(|x| x.id).collect::<Vec<_>>()))),
            ("autor", Arc::new(StringArray::from(c.iter().map(|x| x.autor.as_str()).collect::<Vec<_>>()))),
            ("quando", Arc::new(StringArray::from(c.iter().map(|x| x.quando.as_str()).collect::<Vec<_>>()))),
            ("texto", Arc::new(StringArray::from(c.iter().map(|x| x.texto.as_str()).collect::<Vec<_>>()))),
            ("iniciais", Arc::new(StringArray::from(c.iter().map(|x| x.iniciais.as_str()).collect::<Vec<_>>()))),
            ("resolvido", Arc::new(BooleanArray::from(c.iter().map(|x| x.resolvido).collect::<Vec<_>>()))),
        ],
        HashMap::new(),
    )
}

fn respostas_arrow(c: &[Comentario]) -> Result<Vec<u8>, String> {
    let r: Vec<(u64, &Resposta)> = c.iter().flat_map(|x| x.respostas.iter().map(move |r| (x.id, r))).collect();
    tabela(
        vec![
            ("comentario", Arc::new(UInt64Array::from(r.iter().map(|x| x.0).collect::<Vec<_>>()))),
            ("autor", Arc::new(StringArray::from(r.iter().map(|x| x.1.autor.as_str()).collect::<Vec<_>>()))),
            ("quando", Arc::new(StringArray::from(r.iter().map(|x| x.1.quando.as_str()).collect::<Vec<_>>()))),
            ("texto", Arc::new(StringArray::from(r.iter().map(|x| x.1.texto.as_str()).collect::<Vec<_>>()))),
        ],
        HashMap::new(),
    )
}

fn ler_comentarios(c: &Lida, r: Option<&Lida>) -> Result<Vec<Comentario>, String> {
    let (id, autor, quando, texto, resolvido) = (c.col::<UInt64Array>("id")?, c.col::<StringArray>("autor")?, c.col::<StringArray>("quando")?, c.col::<StringArray>("texto")?, c.col::<BooleanArray>("resolvido")?);
    let mut v: Vec<Comentario> = (0..c.linhas())
        // A coluna das iniciais falta nos arquivos de antes dela.
        .map(|i| Comentario { iniciais: c.col::<StringArray>("iniciais").map(|a| a.value(i).to_string()).unwrap_or_default(), id: id.value(i), autor: autor.value(i).into(), quando: quando.value(i).into(), texto: texto.value(i).into(), resolvido: resolvido.value(i), respostas: Vec::new() })
        .collect();
    if let Some(r) = r {
        let (dono, autor, quando, texto) = (r.col::<UInt64Array>("comentario")?, r.col::<StringArray>("autor")?, r.col::<StringArray>("quando")?, r.col::<StringArray>("texto")?);
        for i in 0..r.linhas() {
            if let Some(c) = v.iter_mut().find(|c| c.id == dono.value(i)) {
                c.respostas.push(Resposta { autor: autor.value(i).into(), quando: quando.value(i).into(), texto: texto.value(i).into() });
            }
        }
    }
    Ok(v)
}

/// O objeto preso ao trecho `i` (as colunas `marca…` faltam nos arquivos de antes delas).
fn marca_lida(t: &Lida, i: usize) -> Option<Marca> {
    let tipo = t.col::<UInt8Array>("marca").ok()?.value(i);
    let texto = |c: &str| t.col::<StringArray>(c).ok().and_then(|a| opcional(a, i, |a, i| a.value(i).to_string())).unwrap_or_default();
    let fim = t.col::<BooleanArray>("marca_fim").is_ok_and(|a| a.value(i));
    marca_das_partes(tipo, texto("marca_a"), texto("marca_b"), fim)
}

/// Os estilos, o padrão (a linha de id 0) e o tema; as colunas da fase 1 do .docx faltam nos arquivos de antes dela.
fn ler_estilos(t: &Lida, e: &mut Estilos) -> Result<(), String> {
    let (id, nome, base, bits) = (t.col::<UInt32Array>("id")?, t.col::<StringArray>("nome")?, t.col::<UInt8Array>("base")?, t.col::<UInt8Array>("bits")?);
    let (tamanho, cor, alinhamento) = (t.col::<Float32Array>("tamanho")?, t.col::<UInt32Array>("cor")?, t.col::<UInt8Array>("alinhamento")?);
    let (antes, depois, entrelinha) = (t.col::<Float32Array>("antes")?, t.col::<Float32Array>("depois")?, t.col::<Float32Array>("entrelinha")?);
    let texto = |c: &str, i: usize| t.col::<StringArray>(c).ok().map(|a| a.value(i).to_string()).unwrap_or_default();
    let u32_ = |c: &str, i: usize| t.col::<UInt32Array>(c).ok().map_or(0, |a| a.value(i));
    let f32_ = |c: &str, i: usize| t.col::<Float32Array>(c).ok().map_or(0.0, |a| a.value(i));
    for i in 0..t.linhas() {
        let x = Estilo {
            id: id.value(i),
            nome: nome.value(i).to_string(),
            base: base.value(i),
            bits: bits.value(i),
            tamanho: tamanho.value(i),
            cor: cor.value(i),
            alinhamento: valor_de(&ALINHAMENTOS, alinhamento.value(i)),
            antes: opcional(antes, i, Float32Array::value),
            depois: opcional(depois, i, Float32Array::value),
            entrelinha: entrelinha.value(i),
            codigo: texto("codigo", i),
            caractere: t.col::<BooleanArray>("caractere").is_ok_and(|a| a.value(i)),
            herda: u32_("herda", i),
            proximo: u32_("proximo", i),
            familia: Familia::de(&texto("familia", i)),
            realce: u32_("realce", i),
            posicao: t.col::<Int8Array>("posicao").ok().map_or(0, |a| a.value(i)),
            fundo: u32_("fundo", i),
            recuo_esq: f32_("recuo_esq", i),
            recuo_dir: f32_("recuo_dir", i),
            primeira: f32_("primeira", i),
            mais_letra: de_json(t, "mais_letra", i),
            mais_paragrafo: de_json(t, "mais_paragrafo", i),
            desligados: u32_("desligados", i),
        };
        if x.id == 0 {
            e.padrao = x;
        } else {
            e.lista.push(x);
        }
    }
    e.tema = t.meta("tema").unwrap_or_default().to_string();
    e.de_tabela = t.meta("estilos_de_tabela").unwrap_or_default().to_string();
    e.legendas = t.meta("legendas").filter(|l| !l.is_empty()).map(|l| l.split('\u{1f}').map(str::to_string).collect()).unwrap_or_default();
    Ok(())
}

/// As colunas de texto de uma fonte, na ordem da tabela.
const CAMPOS_DE_FONTE: [&str; 14] = ["chave", "titulo", "subtitulo", "ano", "edicao", "local", "editora", "veiculo", "volume", "numero", "paginas", "url", "acesso", "doi"];

fn campos_de_fonte(f: &Fonte) -> [&str; 14] {
    [&f.chave, &f.titulo, &f.subtitulo, &f.ano, &f.edicao, &f.local, &f.editora, &f.veiculo, &f.volume, &f.numero, &f.paginas, &f.url, &f.acesso, &f.doi]
}

fn fontes_arrow(fontes: &[Fonte]) -> Result<Vec<u8>, String> {
    let mut colunas: Vec<(&str, ArrayRef)> = CAMPOS_DE_FONTE
        .iter()
        .enumerate()
        .map(|(k, nome)| (*nome, Arc::new(StringArray::from(fontes.iter().map(|f| campos_de_fonte(f)[k]).collect::<Vec<_>>())) as ArrayRef))
        .collect();
    let autores: Vec<String> = fontes.iter().map(|f| f.autores.iter().map(|a| format!("{}{ENTRE_NOMES}{}", a.sobrenome, a.nome)).collect::<Vec<_>>().join(&ENTRE_AUTORES.to_string())).collect();
    colunas.push(("autores", Arc::new(StringArray::from(autores))));
    colunas.push(("tipo", Arc::new(UInt8Array::from(fontes.iter().map(|f| codigo_de(&TIPOS_DE_FONTE, &f.tipo)).collect::<Vec<_>>()))));
    colunas.push(("palavras", Arc::new(StringArray::from(fontes.iter().map(|f| f.palavras.as_str()).collect::<Vec<_>>()))));
    colunas.push(("nota", Arc::new(StringArray::from(fontes.iter().map(|f| f.nota.as_str()).collect::<Vec<_>>()))));
    tabela(colunas, HashMap::new())
}

fn ler_fontes(t: &Lida) -> Result<Vec<Fonte>, String> {
    let textos: Vec<&StringArray> = CAMPOS_DE_FONTE.iter().map(|c| t.col::<StringArray>(c)).collect::<Result<_, _>>()?;
    let (autores, tipo) = (t.col::<StringArray>("autores")?, t.col::<UInt8Array>("tipo")?);
    // Coluna nova (18/10/2026): o .fdoz de antes não tem.
    let palavras = t.col::<StringArray>("palavras").ok();
    let nota = t.col::<StringArray>("nota").ok();
    Ok((0..t.linhas())
        .map(|i| {
            let v = |k: usize| textos[k].value(i).to_string();
            let autores = autores.value(i).split(ENTRE_AUTORES).filter(|a| !a.is_empty()).map(|a| {
                let (sobrenome, nome) = a.split_once(ENTRE_NOMES).unwrap_or((a, ""));
                Autor { sobrenome: sobrenome.to_string(), nome: nome.to_string() }
            });
            Fonte {
                chave: v(0), tipo: valor_de(&TIPOS_DE_FONTE, tipo.value(i)), autores: autores.collect(), titulo: v(1), subtitulo: v(2), ano: v(3), edicao: v(4), local: v(5),
                editora: v(6), veiculo: v(7), volume: v(8), numero: v(9), paginas: v(10), url: v(11), acesso: v(12), doi: v(13),
                palavras: palavras.map_or(String::new(), |p| p.value(i).to_string()),
                nota: nota.map_or(String::new(), |p| p.value(i).to_string()),
            }
        })
        .collect())
}

fn ler_marcadores(t: &Lida) -> Result<Vec<Marcador>, String> {
    let (nome, bloco) = (t.col::<StringArray>("nome")?, t.col::<UInt64Array>("bloco")?);
    Ok((0..t.linhas()).map(|i| Marcador { nome: nome.value(i).to_string(), bloco: bloco.value(i) }).collect())
}

#[cfg(test)]
mod testes;
