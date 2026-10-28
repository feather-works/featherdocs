//! O documento: uma árvore de blocos de tipo fechado. Formatação é anotação
//! estrutural no trecho de texto (`estilo`), nunca marcação solta que se acumula.

use serde::{Deserialize, Serialize};

/// Bits de `Trecho::estilo`.
pub const NEGRITO: u8 = 1;
pub const ITALICO: u8 = 2;
pub const CODIGO: u8 = 4;
pub const SUBLINHADO: u8 = 8;
pub const TACHADO: u8 = 16;
/// Fonte com serifa (Liberation Serif); sem este bit e sem `CODIGO`, a fonte é a sem serifa.
pub const SERIFA: u8 = 64;
/// Fórmula LaTeX em linha (`$...$`); o `texto` guarda o LaTeX.
pub const MATEMATICA: u8 = 32;
/// Só no desenho (nunca gravado num `Trecho`): com `MATEMATICA`, a fórmula é em destaque (estilo
/// display, várias linhas), não a de dentro do parágrafo.
pub const EXIBIDA: u8 = 128;

fn zero(n: &u8) -> bool {
    *n == 0
}

fn zero32(n: &u32) -> bool {
    *n == 0
}

fn falso(b: &bool) -> bool {
    !*b
}

/// O nome de uma fonte ("Georgia") ou, com `+` na frente, a fonte do tema do Word ("+majorHAnsi",
/// "+minorHAnsi", resolvida pelo `Estilos::tema`); vazio = a da classe dos bits (`SERIFA`, `CODIGO`).
/// Os nomes ficam num catálogo único para o `Fmt` continuar `Copy` (o motor o copia a cada trecho);
/// são poucos por execução.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Familia(u32);

static FAMILIAS: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());
/// shortcut: o catálogo nunca encolhe e para em 10 mil nomes (o resto fica sem nome, só a classe); trocar
/// por `Arc<str>` no `Fmt` se um dia ele deixar de ser `Copy`.
const MAX_FAMILIAS: usize = 10_000;

impl Familia {
    pub fn de(nome: &str) -> Self {
        let nome = nome.trim();
        if nome.is_empty() {
            return Self(0);
        }
        let mut v = FAMILIAS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(i) = v.iter().position(|n| *n == nome) {
            return Self(i as u32 + 1);
        }
        if v.len() >= MAX_FAMILIAS {
            return Self(0);
        }
        v.push(Box::leak(nome.to_string().into_boxed_str()));
        Self(v.len() as u32)
    }

    pub fn nome(self) -> &'static str {
        if self.0 == 0 {
            return "";
        }
        FAMILIAS.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(self.0 as usize - 1).copied().unwrap_or("")
    }

    pub fn vazia(&self) -> bool {
        self.0 == 0
    }

    /// A fonte do tema (`+majorHAnsi`): o nome do atributo `w:…Theme` do Word.
    pub fn do_tema(self) -> Option<&'static str> {
        self.nome().strip_prefix('+')
    }
}

impl std::fmt::Debug for Familia {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.nome())
    }
}

impl Serialize for Familia {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.nome())
    }
}

impl<'de> Deserialize<'de> for Familia {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self::de(&String::deserialize(d)?))
    }
}

/// A classe (o bit `SERIFA`, `CODIGO` ou nenhum = sem serifa) de uma fonte pelo nome: é por ela que o
/// motor mede e desenha.
pub fn classe_da_familia(nome: &str) -> u8 {
    let f = nome.to_lowercase();
    if ["courier", "consolas", "mono", "menlo"].iter().any(|m| f.contains(m)) {
        CODIGO
    } else if ["times", "serif", "georgia", "cambria", "garamond", "book antiqua", "palatino"].iter().any(|m| f.contains(m)) && !f.contains("sans") {
        SERIFA
    } else {
        0
    }
}

/// Formatação de caractere além dos bits de `estilo`: tamanho, cor, realce e posição. Tudo zero = o padrão do bloco.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Fmt {
    /// Corpo em pontos; 0 = o do tipo de bloco.
    #[serde(default)]
    pub tamanho: f32,
    /// Cor do texto `0xRRGGBBAA`; 0 = automática.
    #[serde(default)]
    pub cor: u32,
    /// Cor do marca-texto `0xRRGGBBAA`; 0 = sem realce.
    #[serde(default)]
    pub realce: u32,
    /// 1 = sobrescrito, -1 = subscrito, 0 = normal.
    #[serde(default)]
    pub posicao: i8,
    /// O nome da fonte; vazio = a da classe dos bits do trecho.
    #[serde(default, skip_serializing_if = "Familia::vazia")]
    pub familia: Familia,
    /// O estilo de caractere (`Estilos::lista`, `caractere`) que o trecho usa; 0 = nenhum. A letra
    /// dele já vem somada nos bits e no `Fmt` do trecho: o id é para a volta ao Word.
    #[serde(default, skip_serializing_if = "zero32")]
    pub estilo_de_caractere: u32,
}

impl Fmt {
    pub fn eh_padrao(&self) -> bool {
        *self == Self::default()
    }
}

/// O caractere que guarda, no texto, o lugar de um objeto preso a ele (nota, citação, campo…):
/// para o cursor, a seleção e o apagar, o objeto é **um caractere só**.
pub const OBJETO: char = '\u{FFFC}';

/// Um objeto preso a um ponto do texto. O trecho que o carrega tem `texto` = [`OBJETO`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Marca {
    /// Nota de rodapé (ou de fim, com `fim`), com o texto dela. O número sai da ordem no documento.
    Nota {
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        fim: bool,
        texto: Vec<Trecho>,
    },
    /// Citação das fontes `chaves` (de `Documento::fontes`), com a página citada.
    Citacao {
        chaves: Vec<String>,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        pagina: String,
    },
    /// Entrada do índice remissivo (não aparece no texto).
    Indice {
        termo: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        sub: String,
    },
    /// Campo que se atualiza sozinho: `autor`, `titulo`, `data`, `pagina`, `paginas`, `arquivo`.
    Campo { campo: String },
    /// O começo (ou o fim, com `fim`) do trecho comentado pelo comentário `id` (`Documento::comentarios`).
    Comentario {
        id: u64,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        fim: bool,
    },
    /// Imagem no meio da linha (um ícone, um logo), apoiada na linha de base, com o tamanho em pontos;
    /// `origem` como a do bloco de imagem.
    Imagem { origem: String, largura: f32, altura: f32 },
    /// Um objeto do Word que o modelo ainda não entende (forma, gráfico, SmartArt, objeto OLE): o XML
    /// original (`w:drawing`, `w:object`) e as partes do pacote que ele aponta, guardados para voltar
    /// iguais ao `.docx` (a sacola de interoperabilidade). A tela mostra um quadro com o `resumo`.
    ObjetoWord {
        xml: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        partes: Vec<ParteWord>,
        resumo: String,
    },
    /// O começo (ou o fim, com `fim`) de um controle de conteúdo do Word (`w:sdt`): `xml` é o
    /// `w:sdtPr` original (vazio no fim); o texto de dentro continua sendo texto. `bloco` = o
    /// controle envolvia parágrafos inteiros, não um pedaço da linha.
    Controle {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        xml: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        fim: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        bloco: bool,
    },
}

/// Uma parte do pacote `.docx` que um `Marca::ObjetoWord` aponta (o `chart1.xml` do gráfico, o
/// `.xlsx` do objeto OLE, as relações deles).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ParteWord {
    /// O `r:id` com que o XML do objeto aponta a parte; vazio = parte apontada por outra parte (as
    /// relações dela e o que elas apontam), gravada no mesmo `caminho`.
    #[serde(default)]
    pub rid: String,
    /// O tipo da relação (o `Type` inteiro).
    #[serde(default)]
    pub tipo: String,
    /// O caminho dentro do ZIP (`word/charts/chart1.xml`) ou, se `externo`, o endereço.
    pub caminho: String,
    #[serde(default)]
    pub externo: bool,
    /// O `ContentType` da parte (vazio = o do `Default` da extensão).
    #[serde(default)]
    pub conteudo: String,
    #[serde(default)]
    pub bytes: Vec<u8>,
}

/// Uma alteração sugerida (controle de alterações): texto inserido ou apagado, por quem e quando.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoDeRevisao {
    Insercao,
    Remocao,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revisao {
    pub tipo: TipoDeRevisao,
    #[serde(default)]
    pub autor: String,
    #[serde(default)]
    pub quando: String,
}

/// Uma resposta num comentário.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resposta {
    #[serde(default)]
    pub autor: String,
    #[serde(default)]
    pub quando: String,
    pub texto: String,
}

/// Um comentário na margem, ligado a um trecho do texto pelas duas marcas `Marca::Comentario`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comentario {
    pub id: u64,
    #[serde(default)]
    pub autor: String,
    #[serde(default)]
    pub quando: String,
    pub texto: String,
    #[serde(default)]
    pub resolvido: bool,
    #[serde(default)]
    pub respostas: Vec<Resposta>,
}

/// Um pedaço de texto com a mesma formatação.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Trecho {
    pub texto: String,
    #[serde(default, skip_serializing_if = "zero")]
    pub estilo: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(default, skip_serializing_if = "Fmt::eh_padrao")]
    pub fmt: Fmt,
    /// O objeto que este trecho carrega (então `texto` é um [`OBJETO`] só).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marca: Option<Box<Marca>>,
    /// O que o objeto mostra agora ("1", "(SILVA, 2023)"); o motor refaz a cada mudança, não se grava.
    #[serde(skip)]
    pub rotulo: String,
    /// Alteração sugerida (texto inserido ou apagado à espera de aceitar ou rejeitar).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<Box<Revisao>>,
}

impl Trecho {
    /// Um trecho sem formatação.
    pub fn simples(texto: &str) -> Self {
        Self { texto: texto.to_string(), ..Self::default() }
    }

    /// O trecho de um objeto preso ao texto.
    pub fn objeto(marca: Marca) -> Self {
        Self { texto: OBJETO.to_string(), marca: Some(Box::new(marca)), ..Self::default() }
    }
}

/// Uma célula de tabela: texto com formatação.
pub type Celula = Vec<Trecho>;

/// O que um bloco é. O tipo é fechado: nada de "quase qualquer coisa" em texto opaco.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Conteudo {
    Titulo { nivel: u8, texto: Vec<Trecho> },
    Paragrafo { texto: Vec<Trecho> },
    /// Item de lista; a lista é a sequência de itens vizinhos (a numeração sai da posição).
    Item { ordenado: bool, nivel: u8, texto: Vec<Trecho> },
    Citacao { texto: Vec<Trecho> },
    Codigo { linguagem: String, codigo: String },
    Tabela { cabecalho: bool, linhas: Vec<Vec<Celula>> },
    Imagem { origem: String, legenda: Vec<Trecho>, largura: f32, altura: f32 },
    Equacao { latex: String },
    /// Sumário automático: gerado dos títulos até `niveis`, com o número da página (e o pontilhado até ele).
    Sumario {
        niveis: u8,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        sem_numeros: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        sem_pontilhado: bool,
        /// 0 = sumário (títulos); 1, 2 = lista de figuras, de tabelas (as legendas desse tipo).
        #[serde(default, skip_serializing_if = "zero")]
        lista: u8,
        /// Como o `\tableofcontents` do LaTeX: fonte com serifa, seção em negrito sem pontilhado e
        /// com espaço antes, o número numa coluna e o título alinhado depois dele.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        latex: bool,
    },
    Regua,
    QuebraDePagina,
    /// Quebra de coluna: o texto segue na coluna seguinte; com uma coluna só, na página seguinte (como no Word).
    QuebraDeColuna,
    /// Quebra de seção (Layout → Quebras): onde a seção seguinte começa.
    QuebraDeSecao { inicio: InicioDeSecao },
    /// Referências (bibliografia) geradas das fontes citadas no estilo do documento; `todas` = também as não citadas.
    Bibliografia {
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        todas: bool,
    },
    /// Índice remissivo gerado das entradas marcadas (`Marca::Indice`), com as páginas.
    IndiceRemissivo,
}

impl Conteudo {
    /// Um sumário até o nível `niveis`, com números de página e pontilhado.
    pub fn sumario(niveis: u8) -> Self {
        Self::Sumario { niveis, sem_numeros: false, sem_pontilhado: false, lista: 0, latex: false }
    }

    /// Todos os textos com formatação do bloco: o texto corrido, cada célula, a legenda da imagem.
    pub fn textos(&self) -> Vec<&Vec<Trecho>> {
        match self {
            Self::Titulo { texto, .. } | Self::Paragrafo { texto } | Self::Item { texto, .. } | Self::Citacao { texto } => vec![texto],
            Self::Tabela { linhas, .. } => linhas.iter().flatten().collect(),
            Self::Imagem { legenda, .. } => vec![legenda],
            _ => Vec::new(),
        }
    }

    /// Como `textos`, para mudar.
    pub fn textos_mut(&mut self) -> Vec<&mut Vec<Trecho>> {
        match self {
            Self::Titulo { texto, .. } | Self::Paragrafo { texto } | Self::Item { texto, .. } | Self::Citacao { texto } => vec![texto],
            Self::Tabela { linhas, .. } => linhas.iter_mut().flatten().collect(),
            Self::Imagem { legenda, .. } => vec![legenda],
            _ => Vec::new(),
        }
    }
}

/// Onde começa a seção depois de uma quebra de seção.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InicioDeSecao {
    #[default]
    ProximaPagina,
    /// Na mesma página.
    Continua,
    /// Na próxima página de número par (uma página em branco entra no meio, se preciso).
    PaginaPar,
    PaginaImpar,
}

/// Quebra de linha dentro do parágrafo (Shift+Enter, Layout → Quebras → Disposição do texto).
/// Não é `\n`, que divide parágrafos ao digitar ou colar.
pub const QUEBRA_DE_LINHA: char = '\u{2028}';

/// Alinhamento horizontal do texto de um parágrafo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Alinhamento {
    #[default]
    Esquerda,
    Centro,
    Direita,
    Justificado,
}

/// O estilo do marcador de um item de lista.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lista {
    /// O padrão do nível (•, ◦, ▪ ou 1.).
    #[default]
    Padrao,
    Circulo,
    Quadrado,
    Seta,
    Tarefa,
    /// 1. 2. 3.
    Numerica,
    /// a) b) c)
    Alfabetica,
    /// I. II. III.
    Romana,
    /// i. ii. iii. (o `\roman*` do LaTeX, o `lowerRoman` do Word)
    RomanaMinuscula,
}

/// Passo de recuo (1,25 cm) em pontos.
pub const PASSO_RECUO: f32 = 35.43;

/// Atributos de parágrafo que valem para o bloco todo: alinhamento e recuos (em pontos).
/// `primeira` soma ao recuo esquerdo só na primeira linha (negativo = deslocamento para fora).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Atrib {
    #[serde(default)]
    pub alinhamento: Alinhamento,
    /// Entrelinha como múltiplo (1,0 · 1,15 · 1,5 · 2,0…); 0 = a padrão (1,15).
    #[serde(default)]
    pub entrelinha: f32,
    /// Espaço antes e depois do parágrafo em pontos; `None` = o do tipo de bloco.
    #[serde(default)]
    pub antes: Option<f32>,
    #[serde(default)]
    pub depois: Option<f32>,
    /// Cor de fundo do parágrafo `0xRRGGBBAA`; 0 = sem fundo.
    #[serde(default)]
    pub fundo: u32,
    /// Bordas: bits 1 esquerda, 2 direita, 4 em cima, 8 embaixo.
    #[serde(default)]
    pub borda: u8,
    /// Cor das bordas `0xRRGGBBAA`; 0 = a do texto (a chamada usa uma cor própria, e mais grossa).
    #[serde(default)]
    pub cor_borda: u32,
    /// Estilo do marcador (só em item de lista).
    #[serde(default)]
    pub lista: Lista,
    /// Tarefa marcada (só quando `lista` é `Tarefa`).
    #[serde(default)]
    pub marcado: bool,
    /// A numeração recomeça em 1 neste item.
    #[serde(default)]
    pub reiniciar: bool,
    #[serde(default)]
    pub recuo_esq: f32,
    #[serde(default)]
    pub recuo_dir: f32,
    #[serde(default)]
    pub primeira: f32,
    /// O estilo com nome do parágrafo (`Estilos::lista`); 0 = só o tipo do bloco.
    #[serde(default)]
    pub estilo: u32,
    /// Título que fica fora da numeração automática.
    #[serde(default)]
    pub sem_numero: bool,
    /// Parágrafo de legenda: 1 = figura, 2 = tabela, 3 = equação (o número sai sozinho: "Figura 1 –").
    #[serde(default)]
    pub legenda: u8,
    /// Cor do marcador do item de lista `0xRRGGBBAA`; 0 = a do texto.
    #[serde(default)]
    pub cor_marcador: u32,
    /// Título onde começam os apêndices (`\appendix`): daqui em diante o nível 1 recomeça em letras (A, B…).
    #[serde(default)]
    pub apendice: bool,
    /// Espaço elástico antes do bloco (o `\vfill` do LaTeX): o que sobra na página se reparte entre eles.
    #[serde(default)]
    pub encher: bool,
}

impl Atrib {
    pub fn eh_padrao(&self) -> bool {
        *self == Self::default()
    }
}

/// Mesclagem horizontal de células de tabela, ancorada na primeira coluna.
/// A linha conserva suas células originais e apenas células vazias podem ser
/// encobertas; o conteúdo do usuário nunca é descartado.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MesclagemTabela {
    pub linha: usize,
    pub coluna: usize,
    pub colunas: usize,
}

impl MesclagemTabela {
    pub fn contem(&self, linha: usize, coluna: usize) -> bool {
        linha == self.linha
            && coluna >= self.coluna
            && coluna < self.coluna.saturating_add(self.colunas)
    }
}

/// Um bloco com identidade estável (o `id` não muda ao mover ou editar).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bloco {
    pub id: u64,
    #[serde(flatten)]
    pub conteudo: Conteudo,
    #[serde(default, skip_serializing_if = "Atrib::eh_padrao")]
    pub atrib: Atrib,
    /// Mesclagens horizontais; ausente em arquivos legados. Não altera o
    /// modelo textual nem os campos serializados dos tipos de conteúdo.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mesclagens: Vec<MesclagemTabela>,
    /// O que o bloco guarda do código LaTeX de onde veio. Legenda: o separador depois do número
    /// (`:` nas classes padrão, `.`), no lugar do " –" do FeatherDocs. Tabela de um `tabular`: as colunas como no código (`@{}llrr@{}`, `|l|c|`) e,
    /// depois de uma quebra de linha, os fios de cada divisa entre linhas separados por `;` (da de
    /// cima da primeira linha à de baixo da última): `T` `\toprule`, `M` `\midrule`, `B` `\bottomrule`,
    /// `H` `\hline`, `C2-3` `\cline{2-3}`, `c2-3` `\cmidrule{2-3}`, separados por `,`. Vazio = tabela do FeatherDocs.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub latex: String,
}

impl Bloco {
    /// Um bloco com os atributos de parágrafo padrão.
    pub fn novo(id: u64, conteudo: Conteudo) -> Self {
        Self { id, conteudo, atrib: Atrib::default(), mesclagens: Vec::new(), latex: String::new() }
    }
}

/// O papel e as margens, em pontos (1 pt = 1/72 polegada).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConfigPagina {
    pub largura: f32,
    pub altura: f32,
    pub margem_sup: f32,
    pub margem_dir: f32,
    pub margem_inf: f32,
    pub margem_esq: f32,
}

impl ConfigPagina {
    /// A4 com margem de 2,54 cm.
    pub fn a4() -> Self {
        Self { largura: 595.28, altura: 841.89, margem_sup: 72.0, margem_dir: 72.0, margem_inf: 72.0, margem_esq: 72.0 }
    }
    /// Carta (EUA) com margem de 2,54 cm.
    pub fn carta() -> Self {
        Self { largura: 612.0, altura: 792.0, ..Self::a4() }
    }
    /// A largura útil do texto.
    pub fn largura_util(&self) -> f32 {
        self.largura - self.margem_esq - self.margem_dir
    }
    /// A altura útil do texto.
    pub fn altura_util(&self) -> f32 {
        self.altura - self.margem_sup - self.margem_inf
    }
}

impl Default for ConfigPagina {
    fn default() -> Self {
        Self::a4()
    }
}

/// Título e autor do documento.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    #[serde(default)]
    pub titulo: String,
    #[serde(default)]
    pub autor: String,
}

/// Os três lugares de texto de um cabeçalho ou rodapé. Os campos `{pagina}`, `{total}`,
/// `{pagina_romano}`, `{titulo}` e `{autor}` são resolvidos a cada página.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Faixas {
    #[serde(default)]
    pub esquerda: String,
    #[serde(default)]
    pub centro: String,
    #[serde(default)]
    pub direita: String,
    /// Fio entre a faixa e o texto (o `\headrulewidth`/`\footrulewidth`), em pt; 0 = sem fio.
    #[serde(default)]
    pub fio: f32,
    /// A letra da faixa: bits de estilo (`SERIFA`…), tamanho em pt (0 = 9) e cor (0 = o cinza de sempre).
    #[serde(default)]
    pub estilo: u8,
    #[serde(default)]
    pub tamanho: f32,
    #[serde(default)]
    pub cor: u32,
    /// Uma imagem no meio da faixa (o logo do `\fancyhead[C]{\includegraphics…}`): `data:` URL ou o
    /// nome do arquivo, e a largura e a altura em pt.
    #[serde(default)]
    pub imagem: String,
    #[serde(default)]
    pub imagem_largura: f32,
    #[serde(default)]
    pub imagem_altura: f32,
}

impl Faixas {
    pub fn vazia(&self) -> bool {
        self.esquerda.is_empty() && self.centro.is_empty() && self.direita.is_empty() && self.imagem.is_empty()
    }
}

fn rodape_padrao() -> Faixas {
    Faixas { centro: "{pagina}".to_string(), ..Faixas::default() }
}

/// O que vale para a página inteira além do papel: cabeçalho, rodapé e plano de fundo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Extras {
    #[serde(default)]
    pub cabecalho: Faixas,
    #[serde(default = "rodape_padrao")]
    pub rodape: Faixas,
    /// A primeira página com o cabeçalho e o rodapé dela (`cabecalho_primeira`/`rodape_primeira`;
    /// vazios = a capa sem nada).
    #[serde(default)]
    pub primeira_diferente: bool,
    #[serde(default)]
    pub cabecalho_primeira: Faixas,
    #[serde(default)]
    pub rodape_primeira: Faixas,
    /// Cor do papel `0xRRGGBBAA`; 0 = branco.
    #[serde(default)]
    pub cor_pagina: u32,
    /// Marca d'água (texto na diagonal); vazio = sem.
    #[serde(default)]
    pub marca: String,
    #[serde(default)]
    pub marca_cor: u32,
    /// Borda da página: cor (0 = sem), espessura e distância da borda do papel, em pontos.
    #[serde(default)]
    pub borda_cor: u32,
    #[serde(default)]
    pub borda_espessura: f32,
    #[serde(default)]
    pub borda_distancia: f32,
    /// Onde o texto fica na altura da página (Layout → Alinhamento vertical).
    #[serde(default)]
    pub vertical: Vertical,
    /// Números de linha na margem esquerda.
    #[serde(default)]
    pub numerar_linhas: NumeracaoDeLinhas,
    /// O número das notas de rodapé e das de fim: 0 = 1, 2, 3 · 1 = a, b, c · 2 = i, ii, iii · 3 = *, †, ‡.
    #[serde(default)]
    pub formato_notas: u8,
    #[serde(default = "dois")]
    pub formato_notas_fim: u8,
    /// Controle de alterações ligado (Revisão → Modo: Sugerindo): o que se digita ou apaga vira sugestão.
    #[serde(default)]
    pub controlar_alteracoes: bool,
    /// Documento protegido (Revisão → Proteger): só leitura e comentários até desproteger.
    #[serde(default)]
    pub protegido: bool,
    /// Veio do LaTeX: o texto justificado quebra como o TeX (o parágrafo inteiro de uma vez, com o
    /// espaço maior depois do ponto). O de outra origem quebra linha a linha, como o Word.
    #[serde(default)]
    pub composicao_tex: bool,
    /// O número da primeira página (Layout → Número de página → Iniciar em). O `titlepage` do LaTeX
    /// dá 0: a capa não conta e a página seguinte é a 1.
    #[serde(default = "um")]
    pub pagina_inicial: u32,
}

fn um() -> u32 {
    1
}

fn dois() -> u8 {
    2
}

/// O alinhamento vertical do texto na página.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Vertical {
    #[default]
    Topo,
    Centro,
    Base,
}

/// Números de linha na margem.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NumeracaoDeLinhas {
    #[default]
    Nenhuma,
    /// 1, 2, 3… do começo ao fim do documento.
    Continua,
    /// Recomeça em 1 a cada página.
    PorPagina,
}

impl Default for Extras {
    fn default() -> Self {
        Self { cabecalho: Faixas::default(), rodape: rodape_padrao(), primeira_diferente: false, cabecalho_primeira: Faixas::default(), rodape_primeira: Faixas::default(), cor_pagina: 0, marca: String::new(), marca_cor: 0, borda_cor: 0, borda_espessura: 0.0, borda_distancia: 0.0, vertical: Vertical::Topo, numerar_linhas: NumeracaoDeLinhas::Nenhuma, formato_notas: 0, formato_notas_fim: 2, controlar_alteracoes: false, protegido: false, composicao_tex: false, pagina_inicial: 1 }
    }
}

/// A numeração automática dos títulos.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Numeracao {
    #[default]
    Nenhuma,
    /// 1 · 1.1 · 1.1.1
    Decimal,
    /// Capítulo 1 · 1.1 · 1.1.1
    Capitulo,
}

/// Um estilo com nome, como no Word: de parágrafo (criado pelo usuário, `FDEstilo<id>`, ou vindo do
/// `.docx`) ou de caractere. A base diz o tipo do bloco (e o nível no sumário); o resto é o padrão
/// do texto, que a formatação feita à mão continua podendo mudar. Os campos guardam só o que o
/// estilo define; a herança (`herda`) e o padrão do documento se somam em `Estilos::resolvido`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Estilo {
    pub id: u32,
    pub nome: String,
    /// 0 = parágrafo; 1 a 6 = Título 1 a 6.
    pub base: u8,
    /// Bits de `Trecho::estilo` que valem para o parágrafo todo (NEGRITO, ITALICO, SUBLINHADO, SERIFA, CODIGO).
    #[serde(default)]
    pub bits: u8,
    /// Corpo em pontos; 0 = o da base.
    #[serde(default)]
    pub tamanho: f32,
    /// `0xRRGGBBAA`; 0 = automática.
    #[serde(default)]
    pub cor: u32,
    #[serde(default)]
    pub alinhamento: Alinhamento,
    /// Espaço antes e depois em pontos; `None` = o da base.
    #[serde(default)]
    pub antes: Option<f32>,
    #[serde(default)]
    pub depois: Option<f32>,
    /// 0 = a padrão.
    #[serde(default)]
    pub entrelinha: f32,
    /// O `styleId` do Word ("Destaque", "Heading1"); vazio = estilo do FeatherDocs (`FDEstilo<id>`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub codigo: String,
    /// Estilo de caractere (`w:rStyle`): só a letra vale, e ele não entra na lista de parágrafos.
    #[serde(default, skip_serializing_if = "falso")]
    pub caractere: bool,
    /// O estilo de que este herda (`w:basedOn`); 0 = só o padrão do documento.
    #[serde(default, skip_serializing_if = "zero32")]
    pub herda: u32,
    /// O estilo do parágrafo seguinte (`w:next`); 0 = o Normal.
    #[serde(default, skip_serializing_if = "zero32")]
    pub proximo: u32,
    /// O resto do caractere: o nome da fonte, o realce e a posição.
    #[serde(default, skip_serializing_if = "Familia::vazia")]
    pub familia: Familia,
    #[serde(default, skip_serializing_if = "zero32")]
    pub realce: u32,
    #[serde(default, skip_serializing_if = "zero_i8")]
    pub posicao: i8,
    /// O resto do parágrafo: fundo e recuos (em pontos).
    #[serde(default, skip_serializing_if = "zero32")]
    pub fundo: u32,
    #[serde(default, skip_serializing_if = "zero_f32")]
    pub recuo_esq: f32,
    #[serde(default, skip_serializing_if = "zero_f32")]
    pub recuo_dir: f32,
    #[serde(default, skip_serializing_if = "zero_f32")]
    pub primeira: f32,
}

fn zero_i8(n: &i8) -> bool {
    *n == 0
}

fn zero_f32(n: &f32) -> bool {
    *n == 0.0
}

impl Estilo {
    /// A letra do título vindo do LaTeX, guardada em `Bloco.latex` como "corpo bits cor antes depois"
    /// (a classe e o `\titleformat` decidem; não é um estilo com nome do usuário).
    pub fn do_latex(nivel: u8, latex: &str) -> Option<Estilo> {
        let [tamanho, bits, cor, antes, depois] = latex.split(' ').collect::<Vec<_>>()[..] else { return None };
        Some(Estilo {
            id: 0,
            nome: String::new(),
            base: nivel,
            bits: bits.parse().ok()?,
            tamanho: tamanho.parse().ok()?,
            cor: u32::from_str_radix(cor, 16).ok()?,
            alinhamento: Alinhamento::default(),
            antes: antes.parse().ok(),
            depois: depois.parse().ok(),
            ..Estilo::default()
        })
    }

    pub fn eh_padrao(&self) -> bool {
        *self == Self::default()
    }

    /// A letra do estilo como a de um trecho: os bits e o `Fmt`.
    pub fn caractere_do_estilo(&self) -> (u8, Fmt) {
        (self.bits, Fmt { tamanho: self.tamanho, cor: self.cor, realce: self.realce, posicao: self.posicao, familia: self.familia, estilo_de_caractere: 0 })
    }

    pub fn por_caractere(&mut self, (bits, f): (u8, Fmt)) {
        (self.bits, self.tamanho, self.cor, self.realce, self.posicao, self.familia) = (bits, f.tamanho, f.cor, f.realce, f.posicao, f.familia);
    }

    /// O que o estilo define do parágrafo, como os atributos de um bloco.
    pub fn paragrafo_do_estilo(&self) -> Atrib {
        Atrib { alinhamento: self.alinhamento, antes: self.antes, depois: self.depois, entrelinha: self.entrelinha, fundo: self.fundo, recuo_esq: self.recuo_esq, recuo_dir: self.recuo_dir, primeira: self.primeira, ..Atrib::default() }
    }

    pub fn por_paragrafo(&mut self, a: &Atrib) {
        (self.alinhamento, self.antes, self.depois, self.entrelinha) = (a.alinhamento, a.antes, a.depois, a.entrelinha);
        (self.fundo, self.recuo_esq, self.recuo_dir, self.primeira) = (a.fundo, a.recuo_esq, a.recuo_dir, a.primeira);
    }

    /// Soma `de` por cima (o filho sobre o pai): o que `de` define ganha. A fonte de `de` troca a classe.
    // shortcut: os bits só ligam (o `<w:b w:val="0"/>` do filho não desliga o negrito do pai) e o
    // alinhamento à esquerda não desfaz o do pai; resolver quando o modelo tiver "desligado".
    fn somar(&mut self, de: &Estilo) {
        const CLASSE: u8 = SERIFA | CODIGO;
        if !de.familia.vazia() || de.bits & CLASSE != 0 {
            self.bits &= !CLASSE;
            self.familia = de.familia;
        }
        self.bits |= de.bits;
        let mais = |a: &mut f32, b: f32| if b != 0.0 { *a = b };
        let mais_u = |a: &mut u32, b: u32| if b != 0 { *a = b };
        mais(&mut self.tamanho, de.tamanho);
        mais(&mut self.entrelinha, de.entrelinha);
        mais(&mut self.recuo_esq, de.recuo_esq);
        mais(&mut self.recuo_dir, de.recuo_dir);
        mais(&mut self.primeira, de.primeira);
        mais_u(&mut self.cor, de.cor);
        mais_u(&mut self.realce, de.realce);
        mais_u(&mut self.fundo, de.fundo);
        if de.posicao != 0 {
            self.posicao = de.posicao;
        }
        if de.alinhamento != Alinhamento::Esquerda {
            self.alinhamento = de.alinhamento;
        }
        self.antes = de.antes.or(self.antes);
        self.depois = de.depois.or(self.depois);
    }
}

/// Os estilos com nome e a numeração dos títulos.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Estilos {
    #[serde(default)]
    pub numeracao: Numeracao,
    #[serde(default)]
    pub lista: Vec<Estilo>,
    /// O padrão do documento (`w:docDefaults` do Word): a letra e o parágrafo por baixo de todo estilo.
    #[serde(default, skip_serializing_if = "Estilo::eh_padrao")]
    pub padrao: Estilo,
    /// O tema do Word (`theme1.xml`) como veio: dele saem as fontes de tema (`+majorHAnsi`). Vazio = sem tema.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tema: String,
}

impl Estilos {
    pub fn eh_padrao(&self) -> bool {
        *self == Self::default()
    }

    /// O estilo `id` (0 ou um id que não existe mais = nenhum).
    pub fn pegar(&self, id: u32) -> Option<&Estilo> {
        if id == 0 {
            return None;
        }
        self.lista.iter().find(|e| e.id == id)
    }
    /// O estilo com nome que vale para o bloco: só se o tipo dele ainda é o da base do estilo
    /// (o parágrafo que nasceu de um Enter num título com estilo não herda a letra do título).
    pub fn do_bloco(&self, b: &Bloco) -> Option<&Estilo> {
        self.pegar(b.atrib.estilo).filter(|e| !e.caractere && base_do_conteudo(&b.conteudo) == Some(e.base))
    }

    /// O estilo `id` com a herança somada (do mais distante ao mais perto) sobre o padrão do documento
    /// (o de caractere não leva o padrão: ele se soma à letra do parágrafo).
    pub fn resolvido(&self, id: u32) -> Option<Estilo> {
        let e = self.pegar(id)?;
        let mut cadeia = vec![e];
        while let Some(pai) = self.pegar(cadeia[cadeia.len() - 1].herda) {
            if cadeia.len() >= 16 || cadeia.iter().any(|x| x.id == pai.id) {
                break;
            }
            cadeia.push(pai);
        }
        let mut r = if e.caractere { Estilo::default() } else { self.padrao.clone() };
        for x in cadeia.iter().rev() {
            r.somar(x);
        }
        Some(Estilo { id: e.id, nome: e.nome.clone(), base: e.base, codigo: e.codigo.clone(), caractere: e.caractere, herda: e.herda, proximo: e.proximo, ..r })
    }

    /// O estilo resolvido com que o bloco se desenha: o dele; no parágrafo sem estilo, o Normal do
    /// documento (o vindo do Word, ou só o padrão), se o documento tiver um.
    pub fn resolvido_do_bloco(&self, b: &Bloco) -> Option<Estilo> {
        if let Some(e) = self.do_bloco(b) {
            return self.resolvido(e.id);
        }
        if !matches!(b.conteudo, Conteudo::Paragrafo { .. }) {
            return None;
        }
        match self.lista.iter().find(|e| e.codigo == "Normal" && !e.caractere) {
            Some(n) => self.resolvido(n.id),
            None if !self.padrao.eh_padrao() => Some(self.padrao.clone()),
            None => None,
        }
    }

    /// As fontes de texto latino do tema (`a:majorFont` e `a:minorFont`), para resolver `+majorHAnsi`.
    pub fn fontes_do_tema(&self) -> (String, String) {
        let fonte = |grupo: &str| -> String {
            let Some(i) = self.tema.find(&format!("{grupo}>")) else { return String::new() };
            let resto = &self.tema[i..];
            let Some(j) = resto.find("typeface=\"") else { return String::new() };
            resto[j + 10..].split('"').next().unwrap_or("").to_string()
        };
        (fonte("majorFont"), fonte("minorFont"))
    }

    /// O nome de verdade da família (a do tema resolvida); vazio se não há.
    pub fn nome_da_familia(&self, f: Familia) -> String {
        match f.do_tema() {
            Some(t) => {
                let (maior, menor) = self.fontes_do_tema();
                if t.starts_with("major") { maior } else { menor }
            }
            None => f.nome().to_string(),
        }
    }
}

/// 0 para parágrafo, o nível para título; os outros tipos não recebem estilo com nome.
pub fn base_do_conteudo(c: &Conteudo) -> Option<u8> {
    match c {
        Conteudo::Paragrafo { .. } => Some(0),
        Conteudo::Titulo { nivel, .. } => Some(*nivel),
        _ => None,
    }
}

/// O documento inteiro.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Documento {
    pub meta: Meta,
    pub pagina: ConfigPagina,
    pub blocos: Vec<Bloco>,
    pub proximo_id: u64,
    #[serde(default)]
    pub extras: Extras,
    #[serde(default)]
    pub estilos: Estilos,
    /// Os marcadores (Inserir → Marcador): um nome para um bloco, alvo de referência cruzada.
    #[serde(default)]
    pub marcadores: Vec<Marcador>,
    /// As fontes citadas e o estilo das citações.
    #[serde(default)]
    pub bibliografia: Bibliografia,
    /// Os comentários (o trecho de cada um é marcado no texto por `Marca::Comentario`).
    #[serde(default)]
    pub comentarios: Vec<Comentario>,
}

/// Um autor de uma fonte.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Autor {
    pub sobrenome: String,
    #[serde(default)]
    pub nome: String,
}

/// O tipo de uma fonte (muda a forma da referência).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoDeFonte {
    #[default]
    Livro,
    Capitulo,
    Artigo,
    Site,
    Tese,
    Relatorio,
    Outro,
}

/// Uma fonte (obra citada) do documento. `veiculo` é o periódico (artigo), o livro (capítulo) ou o
/// programa/curso (tese).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Fonte {
    /// O nome curto pelo qual o texto a cita ("silva2023"), como no BibTeX.
    pub chave: String,
    #[serde(default)]
    pub tipo: TipoDeFonte,
    #[serde(default)]
    pub autores: Vec<Autor>,
    #[serde(default)]
    pub titulo: String,
    #[serde(default)]
    pub subtitulo: String,
    #[serde(default)]
    pub ano: String,
    #[serde(default)]
    pub edicao: String,
    #[serde(default)]
    pub local: String,
    #[serde(default)]
    pub editora: String,
    #[serde(default)]
    pub veiculo: String,
    #[serde(default)]
    pub volume: String,
    #[serde(default)]
    pub numero: String,
    #[serde(default)]
    pub paginas: String,
    #[serde(default)]
    pub url: String,
    /// Data de acesso (de página da internet), como o usuário escreve.
    #[serde(default)]
    pub acesso: String,
    #[serde(default)]
    pub doi: String,
    /// As palavras-chave (o `keywords` do BibTeX), separadas por vírgula: a lista filtrada as escolhe.
    #[serde(default)]
    pub palavras: String,
    /// O `note` do BibTeX: sai como está, depois do ano.
    #[serde(default)]
    pub nota: String,
}

/// O estilo das citações e das referências.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstiloDeCitacao {
    /// NBR 10520 (autor-data) e NBR 6023.
    #[default]
    Abnt,
    Apa,
    Vancouver,
    Ieee,
    Chicago,
}

/// As fontes do documento e o estilo em que são citadas.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Bibliografia {
    #[serde(default)]
    pub estilo: EstiloDeCitacao,
    #[serde(default)]
    pub fontes: Vec<Fonte>,
    /// Sem "[S. l.]" e "[s. n.]" quando falta o local ou a editora (a opção `noslsn` do biblatex-abnt).
    #[serde(default)]
    pub sem_sl_sn: bool,
}

/// Um marcador: o nome e o bloco para onde ele aponta.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marcador {
    pub nome: String,
    pub bloco: u64,
}

impl Default for Documento {
    /// Um documento novo: um parágrafo vazio.
    fn default() -> Self {
        let mut d = Self { meta: Meta::default(), pagina: ConfigPagina::a4(), blocos: Vec::new(), proximo_id: 1, extras: Extras::default(), estilos: Estilos::default(), marcadores: Vec::new(), bibliografia: Bibliografia::default(), comentarios: Vec::new() };
        d.acrescentar(Conteudo::Paragrafo { texto: Vec::new() });
        d
    }
}

impl Documento {
    /// Um documento sem nenhum bloco (para quem vai preenchê-lo, como os leitores de arquivo).
    pub fn vazio() -> Self {
        Self { meta: Meta::default(), pagina: ConfigPagina::a4(), blocos: Vec::new(), proximo_id: 1, extras: Extras::default(), estilos: Estilos::default(), marcadores: Vec::new(), bibliografia: Bibliografia::default(), comentarios: Vec::new() }
    }

    /// Dá um `id` novo a um bloco.
    pub fn novo_bloco(&mut self, conteudo: Conteudo) -> Bloco {
        let id = self.proximo_id;
        self.proximo_id += 1;
        Bloco::novo(id, conteudo)
    }

    /// Acrescenta um bloco no fim e devolve o `id`.
    pub fn acrescentar(&mut self, conteudo: Conteudo) -> u64 {
        let b = self.novo_bloco(conteudo);
        let id = b.id;
        self.blocos.push(b);
        id
    }

    /// A posição do bloco com este `id`.
    pub fn indice(&self, id: u64) -> Option<usize> {
        self.blocos.iter().position(|b| b.id == id)
    }

    /// Garante que `proximo_id` é maior que todo `id` (depois de ler de um arquivo).
    pub fn acertar_ids(&mut self) {
        let maior = self.blocos.iter().map(|b| b.id).max().unwrap_or(0);
        self.proximo_id = self.proximo_id.max(maior + 1);
    }
}
