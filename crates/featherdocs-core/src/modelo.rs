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
    /// O resto do `w:rPr` do Word (versalete, tipo do sublinhado, espaçamento…).
    #[serde(default, skip_serializing_if = "MaisLetra::eh_padrao")]
    pub mais: MaisLetra,
}

/// Bits de `MaisLetra::efeitos`.
pub const MAIUSCULAS: u8 = 1;
pub const VERSALETE: u8 = 2;
pub const TACHADO_DUPLO: u8 = 4;
pub const CONTORNO: u8 = 8;
pub const SOMBRA: u8 = 16;
pub const RELEVO: u8 = 32;
pub const BAIXO_RELEVO: u8 = 64;
pub const OCULTO: u8 = 128;

/// Os tipos de sublinhado do Word (`w:u`), na ordem de `MaisLetra::sublinhado`.
pub const TIPOS_DE_SUBLINHADO: [&str; 17] = [
    "single", "words", "double", "thick", "dotted", "dottedHeavy", "dash", "dashedHeavy", "dashLong", "dashLongHeavy", "dotDash", "dashDotHeavy", "dotDotDash", "dashDotDotHeavy", "wave", "wavyHeavy", "wavyDouble",
];

/// O que o caractere do Word tem além do `Fmt` de sempre. Tudo zero = nada a mais.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MaisLetra {
    /// `MAIUSCULAS`, `VERSALETE`, `TACHADO_DUPLO`, `CONTORNO`, `SOMBRA`, `RELEVO`, `BAIXO_RELEVO`, `OCULTO`.
    #[serde(default, skip_serializing_if = "zero")]
    pub efeitos: u8,
    /// Com o bit `SUBLINHADO`: o tipo, índice em `TIPOS_DE_SUBLINHADO` (0 = simples).
    #[serde(default, skip_serializing_if = "zero")]
    pub sublinhado: u8,
    /// A cor do sublinhado `0xRRGGBBAA`; 0 = a do texto.
    #[serde(default, skip_serializing_if = "zero32")]
    pub cor_sublinhado: u32,
    /// Quanto o texto sobe (+) ou desce (−) em pontos, sem mudar o corpo (`w:position`).
    #[serde(default, skip_serializing_if = "zero_f32")]
    pub elevacao: f32,
    /// Espaço a mais (ou a menos) entre as letras, em pontos.
    #[serde(default, skip_serializing_if = "zero_f32")]
    pub espacamento: f32,
    /// A largura das letras em %; 0 = 100.
    #[serde(default, skip_serializing_if = "zero_f32")]
    pub escala: f32,
    /// O corpo, em pontos, a partir do qual os pares de letras se ajustam (`w:kern`); 0 = sem.
    #[serde(default, skip_serializing_if = "zero_f32")]
    pub kerning: f32,
    /// A cor do tema de que `Fmt::cor` saiu (`w:themeColor`), o código do mapa do `.docx`; 0 = cor fixa.
    #[serde(default, skip_serializing_if = "zero")]
    pub cor_tema: u8,
    /// O marca-texto do Word (`w:highlight`) de que `Fmt::realce` saiu, o código do mapa; 0 = sombreamento.
    #[serde(default, skip_serializing_if = "zero")]
    pub marca_texto: u8,
}

impl MaisLetra {
    pub fn eh_padrao(&self) -> bool {
        *self == Self::default()
    }

    /// Soma `de` por cima: os efeitos se juntam (menos os que `desligados` tira), e o que `de` define ganha.
    pub fn somar(&mut self, de: &MaisLetra, desligados: u8) {
        self.efeitos = (self.efeitos & !desligados) | de.efeitos;
        if de.sublinhado != 0 || de.cor_sublinhado != 0 {
            (self.sublinhado, self.cor_sublinhado) = (de.sublinhado, de.cor_sublinhado);
        }
        for (a, b) in [(&mut self.elevacao, de.elevacao), (&mut self.espacamento, de.espacamento), (&mut self.escala, de.escala), (&mut self.kerning, de.kerning)] {
            if b != 0.0 {
                *a = b;
            }
        }
        for (a, b) in [(&mut self.cor_tema, de.cor_tema), (&mut self.marca_texto, de.marca_texto)] {
            if b != 0 {
                *a = b;
            }
        }
    }
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
    /// A numeração recomeça neste item (em 1; na lista do Word, no início do nível).
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
    /// O resto do `w:pPr` do Word (manter com o próximo, entrelinha exata, tabulações…).
    #[serde(default, skip_serializing_if = "MaisParagrafo::eh_padrao")]
    pub mais: MaisParagrafo,
}

/// Bits de `MaisParagrafo::fluxo`.
pub const MANTER_COM_PROXIMO: u8 = 1;
pub const LINHAS_JUNTAS: u8 = 2;
/// Quebra de página antes (`w:pageBreakBefore`); a quebra manual (`w:br`, Ctrl+Enter) é o bloco `QuebraDePagina`.
pub const QUEBRA_ANTES: u8 = 4;
/// Sem o controle de linhas órfãs e viúvas (`w:widowControl w:val="0"`).
pub const VIUVAS_LIVRES: u8 = 8;
/// Não soma o espaço entre parágrafos do mesmo estilo (`w:contextualSpacing`).
pub const ESPACO_CONTEXTUAL: u8 = 16;
pub const ANTES_AUTOMATICO: u8 = 32;
pub const DEPOIS_AUTOMATICO: u8 = 64;
/// Com `Alinhamento::Justificado`: justificado distribuído (a última linha também estica).
pub const DISTRIBUIDO: u8 = 128;

/// Uma parada de tabulação: a posição em pontos (da margem do parágrafo), e os códigos do mapa do
/// `.docx` do alinhamento (0 = esquerda) e do preenchimento (0 = nenhum).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Tabulacao {
    pub posicao: f32,
    #[serde(default)]
    pub alinhamento: u8,
    #[serde(default)]
    pub preenchimento: u8,
}

/// shortcut: até 16 paradas por parágrafo (o `Atrib` é `Copy`); o Word aceita 64, passar para `Vec` se faltar.
pub const MAX_TABULACOES: usize = 16;

/// O que o parágrafo do Word tem além do `Atrib` de sempre. Tudo zero = nada a mais.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MaisParagrafo {
    /// `MANTER_COM_PROXIMO`, `LINHAS_JUNTAS`, `QUEBRA_ANTES`, `VIUVAS_LIVRES`, `ESPACO_CONTEXTUAL`,
    /// `ANTES_AUTOMATICO`, `DEPOIS_AUTOMATICO`, `DISTRIBUIDO`.
    #[serde(default, skip_serializing_if = "zero")]
    pub fluxo: u8,
    /// Entrelinha "pelo menos" e exata, em pontos; 0 = a múltipla de `Atrib::entrelinha`.
    #[serde(default, skip_serializing_if = "zero_f32")]
    pub entrelinha_minima: f32,
    #[serde(default, skip_serializing_if = "zero_f32")]
    pub entrelinha_exata: f32,
    /// O traço das bordas (código do mapa do `.docx`; 0 = simples) e a espessura em pontos (0 = 0,75).
    // shortcut: um traço e uma espessura para os quatro lados (o Word tem um por lado); separar se aparecer documento que use.
    #[serde(default, skip_serializing_if = "zero")]
    pub borda_tipo: u8,
    #[serde(default, skip_serializing_if = "zero_f32")]
    pub borda_espessura: f32,
    /// O nível de tópico (`w:outlineLvl`) de um parágrafo que não é título: 1 a 9; 0 = texto.
    #[serde(default, skip_serializing_if = "zero")]
    pub nivel_topico: u8,
    /// Letra capitular (`w:framePr w:dropCap`): quantas linhas ela desce (+ 128 = na margem); 0 = não é.
    #[serde(default, skip_serializing_if = "zero")]
    pub capitular: u8,
    /// As paradas de tabulação, na ordem; posição 0 = sem parada (fim da lista).
    #[serde(default, skip_serializing_if = "sem_tabulacoes")]
    pub tabulacoes: [Tabulacao; MAX_TABULACOES],
    /// A lista do Word (`Estilos::listas`, a partir de 1) que numera o item; 0 = a lista do FeatherDocs
    /// (a sequência de itens vizinhos, recomeça depois de um parágrafo). A do Word conta pelo documento
    /// todo, atravessando parágrafos, como no Word.
    #[serde(default, skip_serializing_if = "zero16")]
    pub lista: u16,
    /// Num estilo de parágrafo com `lista`: o nível em que ele põe o parágrafo (o `w:ilvl` do estilo).
    /// No item, o nível é o de `Conteudo::Item`.
    #[serde(default, skip_serializing_if = "zero")]
    pub nivel_lista: u8,
    /// O número deste item (a contagem segue dele); `None` = o que vem da contagem.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inicio: Option<u32>,
}

fn zero16(n: &u16) -> bool {
    *n == 0
}

/// Uma lista do Word (`w:abstractNum`): como cada nível numera. Os itens a apontam por
/// `MaisParagrafo::lista`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ListaDoWord {
    #[serde(default)]
    pub niveis: Vec<NivelDeLista>,
    /// O estilo de lista (`w:numStyleLink`, o `styleId`) de onde os níveis vieram; vazio = a lista se define.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub estilo: String,
}

/// Um nível de uma lista do Word (`w:lvl`).
// shortcut: o rPr do número além da fonte (cor, negrito), `w:suff`, `w:lvlJc`, `w:lvlRestart` e o `w:lvlOverride` com nível inteiro não entram; entram quando aparecer documento que precise.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NivelDeLista {
    /// O `w:numFmt` (`decimal`, `lowerLetter`, `upperRoman`, `bullet`, `none`…).
    #[serde(default)]
    pub formato: String,
    /// O `w:lvlText`: `%1` é o número do nível 1, `%2` o do 2…; no marcador, o caractere.
    #[serde(default)]
    pub texto: String,
    /// O `w:start`.
    #[serde(default)]
    pub inicio: u32,
    /// O recuo do nível e o deslocamento da primeira linha (o número), em pontos.
    // shortcut: só vão e voltam; o motor recua o item 18 pt por nível, como na lista do FeatherDocs.
    #[serde(default)]
    pub recuo: f32,
    #[serde(default)]
    pub pendurado: f32,
    /// A fonte do número ou do marcador (a Wingdings e a Symbol mudam o que o caractere é); vazio = a do texto.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fonte: String,
}

fn sem_tabulacoes(t: &[Tabulacao; MAX_TABULACOES]) -> bool {
    t[0] == Tabulacao::default()
}

impl MaisParagrafo {
    pub fn eh_padrao(&self) -> bool {
        *self == Self::default()
    }

    /// As paradas de tabulação definidas.
    pub fn paradas(&self) -> &[Tabulacao] {
        let n = self.tabulacoes.iter().position(|t| *t == Tabulacao::default()).unwrap_or(MAX_TABULACOES);
        &self.tabulacoes[..n]
    }

    /// Acrescenta uma parada; a que já havia na mesma posição (a do estilo) é trocada, e a que passa do
    /// limite fica de fora.
    pub fn acrescentar_tabulacao(&mut self, t: Tabulacao) {
        if let Some(lugar) = self.tabulacoes.iter_mut().find(|x| x.posicao == t.posicao || **x == Tabulacao::default()) {
            *lugar = t;
        }
    }

    /// Soma `de` por cima, como `MaisLetra::somar`.
    // shortcut: as tabulações do filho trocam as do pai inteiras (o Word soma e tira com `clear`).
    pub fn somar(&mut self, de: &MaisParagrafo, desligados: u8) {
        self.fluxo = (self.fluxo & !desligados) | de.fluxo;
        for (a, b) in [(&mut self.entrelinha_minima, de.entrelinha_minima), (&mut self.entrelinha_exata, de.entrelinha_exata), (&mut self.borda_espessura, de.borda_espessura)] {
            if b != 0.0 {
                *a = b;
            }
        }
        for (a, b) in [(&mut self.borda_tipo, de.borda_tipo), (&mut self.nivel_topico, de.nivel_topico), (&mut self.capitular, de.capitular)] {
            if b != 0 {
                *a = b;
            }
        }
        if !de.paradas().is_empty() {
            self.tabulacoes = de.tabulacoes;
        }
        if de.lista != 0 {
            (self.lista, self.nivel_lista) = (de.lista, de.nivel_lista);
        }
    }
}

impl Atrib {
    pub fn eh_padrao(&self) -> bool {
        *self == Self::default()
    }

    /// Tira o que é de item de lista (o estilo do marcador, a lista do Word, o reinício).
    pub fn limpar_lista(&mut self) {
        self.lista = Lista::Padrao;
        self.marcado = false;
        self.reiniciar = false;
        self.mais.lista = 0;
        self.mais.inicio = None;
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

/// Um fio de borda de tabela ou de célula, como o `w:top`/`w:insideH` do Word.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FioDeBorda {
    /// O `w:val` do Word: `single`, `double`, `dashed`, `dotted`, `nil` (sem fio)…
    pub estilo: String,
    /// Espessura em oitavos de ponto (o `w:sz`).
    pub espessura: u16,
    /// RGBA; 0 = automática.
    pub cor: u32,
}

/// Os seis fios de uma tabela (`w:tblBorders`) ou os quatro de uma célula (`w:tcBorders`);
/// `None` = não definido (a tabela usa a grade padrão, a célula a da tabela).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BordasDeTabela {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cima: Option<FioDeBorda>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub esquerda: Option<FioDeBorda>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baixo: Option<FioDeBorda>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direita: Option<FioDeBorda>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dentro_h: Option<FioDeBorda>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dentro_v: Option<FioDeBorda>,
}

impl BordasDeTabela {
    pub fn eh_padrao(&self) -> bool {
        *self == Self::default()
    }
}

/// O que uma célula tem além do texto. `linha`/`coluna` são da grade: com mescla, a célula
/// encoberta continua existindo (vazia) no modelo.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PropsCelula {
    pub linha: usize,
    pub coluna: usize,
    /// Fundo (RGBA); 0 = sem.
    #[serde(default, skip_serializing_if = "zero32")]
    pub fundo: u32,
    /// 0 em cima, 1 no meio, 2 embaixo.
    #[serde(default, skip_serializing_if = "zero")]
    pub vertical: u8,
    /// Alinhamento do texto (o `w:jc` do parágrafo da célula).
    #[serde(default, skip_serializing_if = "alinhamento_padrao")]
    pub alinhamento: Alinhamento,
    #[serde(default, skip_serializing_if = "BordasDeTabela::eh_padrao")]
    pub bordas: BordasDeTabela,
    /// Margens internas em pontos (cima, esquerda, baixo, direita); `None` = as da tabela.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margens: Option<[f32; 4]>,
    /// Direção do texto do Word (`btLr`, `tbRl`); vazio = horizontal.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub direcao: String,
    /// Mescla vertical: quantas linhas a célula ocupa a partir desta (as de baixo ficam vazias).
    #[serde(default, skip_serializing_if = "um_ou_menos")]
    pub linhas: usize,
}

fn alinhamento_padrao(a: &Alinhamento) -> bool {
    *a == Alinhamento::Esquerda
}

fn um_ou_menos(n: &usize) -> bool {
    *n <= 1
}

/// Altura mínima (ou exata) de uma linha, em pontos.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AlturaDeLinha {
    pub linha: usize,
    pub altura: f32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub exata: bool,
}

/// As propriedades de tabela do Word que o `Conteudo::Tabela` não tem. O alinhamento da
/// tabela na página é o `atrib.alinhamento` do bloco.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PropsTabela {
    /// Largura de cada coluna em pontos; vazio = todas iguais na largura útil.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub colunas: Vec<f32>,
    /// Largura da tabela em porcentagem da largura útil; 0 = a soma das colunas.
    #[serde(default, skip_serializing_if = "zero_f32")]
    pub largura_pct: f32,
    /// Layout fixo (`w:tblLayout fixed`): as colunas não se ajustam ao conteúdo.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fixa: bool,
    #[serde(default, skip_serializing_if = "BordasDeTabela::eh_padrao")]
    pub bordas: BordasDeTabela,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alturas: Vec<AlturaDeLinha>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub celulas: Vec<PropsCelula>,
}

impl PropsTabela {
    pub fn eh_padrao(&self) -> bool {
        *self == Self::default()
    }

    pub fn celula(&self, linha: usize, coluna: usize) -> Option<&PropsCelula> {
        self.celulas.iter().find(|c| c.linha == linha && c.coluna == coluna)
    }

    /// A da célula, criada vazia se ainda não houver.
    pub fn celula_mut(&mut self, linha: usize, coluna: usize) -> &mut PropsCelula {
        let i = match self.celulas.iter().position(|c| c.linha == linha && c.coluna == coluna) {
            Some(i) => i,
            None => {
                self.celulas.push(PropsCelula { linha, coluna, ..PropsCelula::default() });
                self.celulas.len() - 1
            }
        };
        &mut self.celulas[i]
    }

    pub fn altura(&self, linha: usize) -> Option<&AlturaDeLinha> {
        self.alturas.iter().find(|a| a.linha == linha)
    }

    pub fn tem_mescla_vertical(&self) -> bool {
        self.celulas.iter().any(|c| c.linhas > 1)
    }

    /// A célula de cima que encobre (`linha`, `coluna`) pela mescla vertical, se houver.
    pub fn encoberta_por(&self, linha: usize, coluna: usize) -> Option<&PropsCelula> {
        self.celulas.iter().find(|c| c.coluna == coluna && c.linhas > 1 && c.linha < linha && linha < c.linha + c.linhas)
    }

    /// Uma linha nova em `em`: o que estava dali para baixo desce uma.
    pub fn inserir_linha(&mut self, em: usize) {
        self.celulas.iter_mut().filter(|c| c.linha >= em).for_each(|c| c.linha += 1);
        self.alturas.iter_mut().filter(|a| a.linha >= em).for_each(|a| a.linha += 1);
    }

    pub fn remover_linha(&mut self, l: usize) {
        self.celulas.retain(|c| c.linha != l);
        self.alturas.retain(|a| a.linha != l);
        self.celulas.iter_mut().filter(|c| c.linha > l).for_each(|c| c.linha -= 1);
        self.alturas.iter_mut().filter(|a| a.linha > l).for_each(|a| a.linha -= 1);
    }

    /// Uma coluna nova em `em`, com a largura da vizinha (a soma das larguras cresce junto).
    pub fn inserir_coluna(&mut self, em: usize) {
        self.celulas.iter_mut().filter(|c| c.coluna >= em).for_each(|c| c.coluna += 1);
        if !self.colunas.is_empty() {
            let w = self.colunas[em.min(self.colunas.len() - 1)];
            self.colunas.insert(em.min(self.colunas.len()), w);
        }
    }

    pub fn remover_coluna(&mut self, k: usize) {
        self.celulas.retain(|c| c.coluna != k);
        self.celulas.iter_mut().filter(|c| c.coluna > k).for_each(|c| c.coluna -= 1);
        if k < self.colunas.len() {
            self.colunas.remove(k);
        }
    }

    /// Joga fora o que caiu fora de uma tabela `linhas` × `colunas` (depois de trocar o conteúdo inteiro).
    pub fn ajustar(&mut self, linhas: usize, colunas: usize) {
        self.celulas.retain(|c| c.linha < linhas && c.coluna < colunas);
        self.celulas.iter_mut().for_each(|c| c.linhas = c.linhas.min(linhas - c.linha));
        self.alturas.retain(|a| a.linha < linhas);
        if self.colunas.len() != colunas {
            self.colunas.clear();
        }
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
    /// Larguras, bordas, fundos e o resto do que o Word guarda na tabela (vazio fora de tabela).
    #[serde(default, skip_serializing_if = "PropsTabela::eh_padrao")]
    pub tabela: PropsTabela,
    /// O que o Word guarda da imagem além do arquivo e do tamanho (vazio fora de imagem).
    #[serde(default, skip_serializing_if = "PropsImagem::eh_padrao")]
    pub imagem: PropsImagem,
}

/// As propriedades da imagem do bloco: o texto alternativo (`wp:docPr descr`), lido por leitores de tela.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PropsImagem {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub descricao: String,
}

impl PropsImagem {
    pub fn eh_padrao(&self) -> bool {
        *self == Self::default()
    }
}

impl Bloco {
    /// Um bloco com os atributos de parágrafo padrão.
    pub fn novo(id: u64, conteudo: Conteudo) -> Self {
        Self { id, conteudo, atrib: Atrib::default(), mesclagens: Vec::new(), latex: String::new(), tabela: PropsTabela::default(), imagem: PropsImagem::default() }
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

/// O que o arquivo do Word guarda do documento além de título e autor: `docProps/core.xml`,
/// `app.xml`, `custom.xml`, o idioma padrão (`w:docDefaults`), a hifenização e a tabulação padrão.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MaisDocumento {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub assunto: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub palavras_chave: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub descricao: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub categoria: String,
    /// Criado e modificado, como o Word grava (W3CDTF, `2026-10-28T14:00:00Z`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub criado: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub modificado: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub empresa: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub gerente: String,
    /// Propriedades personalizadas (nome, texto).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub personalizadas: Vec<(String, String)>,
    /// Idioma padrão do texto (`pt-BR`, `en-US`); vazio = pt-BR.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub idioma: String,
    /// Hifenização automática ligada.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hifenizar: bool,
    /// A parada de tabulação padrão, em pontos; 0 = 1,25 cm (a do Word).
    #[serde(default, skip_serializing_if = "zero_f32")]
    pub tabulacao: f32,
    /// Da borda do papel ao cabeçalho e ao rodapé, em pontos (o `w:header`/`w:footer` do Word); 0 = na metade da margem.
    #[serde(default, skip_serializing_if = "zero_f32")]
    pub distancia_cabecalho: f32,
    #[serde(default, skip_serializing_if = "zero_f32")]
    pub distancia_rodape: f32,
    /// Como o número da página aparece (`{pagina}`, `{total}`): o `w:fmt` do Word (`lowerRoman`,
    /// `upperRoman`, `lowerLetter`, `upperLetter`); vazio = 1, 2, 3.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub formato_pagina: String,
    /// Páginas pares com cabeçalho e rodapé próprios (o `w:evenAndOddHeaders` do Word); as ímpares usam os de sempre.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pares_diferentes: bool,
    #[serde(default, skip_serializing_if = "Faixas::vazia")]
    pub cabecalho_par: Faixas,
    #[serde(default, skip_serializing_if = "Faixas::vazia")]
    pub rodape_par: Faixas,
}

/// Da borda do papel à faixa quando a distância é automática (a linha de base na metade da margem,
/// como a tela desenha): a conta que o `.docx` grava e que o leitor reconhece para voltar a 0.
pub fn distancia_automatica(cfg: &ConfigPagina, f: &Faixas, cabecalho: bool) -> f32 {
    let corpo = if f.tamanho > 0.0 { f.tamanho } else { 9.0 };
    if cabecalho {
        (cfg.margem_sup / 2.0 + 3.0 - corpo * 0.9).max(12.0)
    } else {
        (cfg.margem_inf / 2.0 - 3.0 - corpo * 0.25).max(12.0)
    }
}

/// O número `n` no formato de página `fmt` do Word (vazio ou desconhecido = algarismos).
pub fn numero_de_pagina(n: usize, fmt: &str) -> String {
    let letras = || {
        // a … z, aa … zz, como o Word.
        let (k, r) = ((n.max(1) - 1) / 26, (n.max(1) - 1) % 26);
        char::from(b'a' + r as u8).to_string().repeat(k + 1)
    };
    match fmt {
        "lowerRoman" => crate::paginacao::romano(n as u32).to_lowercase(),
        "upperRoman" => crate::paginacao::romano(n as u32).to_uppercase(),
        "lowerLetter" => letras(),
        "upperLetter" => letras().to_uppercase(),
        _ => n.to_string(),
    }
}

impl MaisDocumento {
    pub fn eh_padrao(&self) -> bool {
        *self == Self::default()
    }
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
    /// A tinta da aba Desenhar, uma por página (o texto `FTINTA` do feather-draw, em pontos da
    /// página; vazio = página sem tinta).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tinta: Vec<String>,
    /// Imagens presas à página (enfeite de canto, faixa até a borda do papel), por trás ou na frente do texto.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decoracoes: Vec<Decoracao>,
    /// Propriedades do arquivo, idioma, hifenização e tabulação padrão (as do Word que o resto não tem).
    #[serde(default, skip_serializing_if = "MaisDocumento::eh_padrao")]
    pub documento: MaisDocumento,
}

/// Uma imagem presa à página, como a âncora do Word (`wp:anchor` com posição relativa à página):
/// o texto não anda por causa dela. Medidas em pontos, do canto de cima à esquerda do papel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Decoracao {
    pub origem: String,
    pub x: f32,
    pub y: f32,
    pub largura: f32,
    pub altura: f32,
    /// Atrás do texto (o "atrás do texto" do Word); senão, na frente.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub atras: bool,
    /// O bloco onde ela está ancorada: aparece na página em que ele começa. `None` = em todas as páginas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bloco: Option<u64>,
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
        Self { cabecalho: Faixas::default(), rodape: rodape_padrao(), primeira_diferente: false, cabecalho_primeira: Faixas::default(), rodape_primeira: Faixas::default(), cor_pagina: 0, marca: String::new(), marca_cor: 0, borda_cor: 0, borda_espessura: 0.0, borda_distancia: 0.0, vertical: Vertical::Topo, numerar_linhas: NumeracaoDeLinhas::Nenhuma, formato_notas: 0, formato_notas_fim: 2, controlar_alteracoes: false, protegido: false, composicao_tex: false, pagina_inicial: 1, tinta: Vec::new(), decoracoes: Vec::new(), documento: MaisDocumento::default() }
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
    /// O resto da letra e do parágrafo do Word.
    #[serde(default, skip_serializing_if = "MaisLetra::eh_padrao")]
    pub mais_letra: MaisLetra,
    #[serde(default, skip_serializing_if = "MaisParagrafo::eh_padrao")]
    pub mais_paragrafo: MaisParagrafo,
    /// O que o estilo define como **desligado** (o `<w:b w:val="0"/>`), que desfaz o do pai: os bits
    /// de `Trecho::estilo` (bits 0–7), os `MaisLetra::efeitos` (8–15), o `MaisParagrafo::fluxo`
    /// (16–23) e `ALINHAMENTO_DEFINIDO`.
    #[serde(default, skip_serializing_if = "zero32")]
    pub desligados: u32,
}

/// Bit de `Estilo::desligados`: o alinhamento à esquerda é do estilo (desfaz o do pai).
pub const ALINHAMENTO_DEFINIDO: u32 = 1 << 24;

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
        (self.bits, Fmt { tamanho: self.tamanho, cor: self.cor, realce: self.realce, posicao: self.posicao, familia: self.familia, estilo_de_caractere: 0, mais: self.mais_letra })
    }

    pub fn por_caractere(&mut self, (bits, f): (u8, Fmt)) {
        (self.bits, self.tamanho, self.cor, self.realce, self.posicao, self.familia, self.mais_letra) = (bits, f.tamanho, f.cor, f.realce, f.posicao, f.familia, f.mais);
    }

    /// O que o estilo define do parágrafo, como os atributos de um bloco.
    pub fn paragrafo_do_estilo(&self) -> Atrib {
        Atrib { alinhamento: self.alinhamento, antes: self.antes, depois: self.depois, entrelinha: self.entrelinha, fundo: self.fundo, recuo_esq: self.recuo_esq, recuo_dir: self.recuo_dir, primeira: self.primeira, mais: self.mais_paragrafo, ..Atrib::default() }
    }

    pub fn por_paragrafo(&mut self, a: &Atrib) {
        (self.alinhamento, self.antes, self.depois, self.entrelinha) = (a.alinhamento, a.antes, a.depois, a.entrelinha);
        (self.fundo, self.recuo_esq, self.recuo_dir, self.primeira, self.mais_paragrafo) = (a.fundo, a.recuo_esq, a.recuo_dir, a.primeira, a.mais);
    }

    /// Soma `de` por cima (o filho sobre o pai): o que `de` define ganha, inclusive o que ele desliga
    /// (`desligados`). A fonte de `de` troca a classe.
    // shortcut: valor zero não desfaz o do pai (o `w:color w:val="auto"` do filho não tira o vermelho do pai).
    fn somar(&mut self, de: &Estilo) {
        const CLASSE: u8 = SERIFA | CODIGO;
        if !de.familia.vazia() || de.bits & CLASSE != 0 {
            self.bits &= !CLASSE;
            self.familia = de.familia;
        }
        let [bits, efeitos, fluxo, _] = de.desligados.to_le_bytes();
        self.bits = (self.bits & !bits) | de.bits;
        self.mais_letra.somar(&de.mais_letra, efeitos);
        self.mais_paragrafo.somar(&de.mais_paragrafo, fluxo);
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
        if de.alinhamento != Alinhamento::Esquerda || de.desligados & ALINHAMENTO_DEFINIDO != 0 {
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
    /// As listas do Word (`numbering.xml`) que os itens apontam (`MaisParagrafo::lista` = índice + 1).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub listas: Vec<ListaDoWord>,
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
