//! Modelos de documento da tela inicial ("Novo"): cada um monta um `Documento` de verdade,
//! com títulos, listas e tabelas do próprio motor (a miniatura é a primeira página dele).
//! As formas geométricas são SVG escritos aqui; as ilustrações (PNG sem fundo) moram em `dados/modelos`.
use crate::inline::analisar;
use crate::modelo::{
    Alinhamento, BordasDeTabela, ConfigPagina, Conteudo, Decoracao, Documento, FioDeBorda, PropsCelula, Trecho, NEGRITO,
    ITALICO, SERIFA,
};

/// O que a galeria mostra de cada modelo.
pub struct ModeloInfo {
    pub id: &'static str,
    pub nome: &'static str,
    pub descricao: &'static str,
}

const MODELOS: [ModeloInfo; 12] = [
    ModeloInfo { id: "em-branco", nome: "Documento em branco", descricao: "Uma página A4 vazia para começar do zero." },
    ModeloInfo { id: "carta", nome: "Carta", descricao: "Cabeçalho em faixa com seu nome e contato, corpo e assinatura." },
    ModeloInfo { id: "relatorio", nome: "Relatório", descricao: "Capa com gráfico, sumário, indicadores em destaque e tabela de resultados." },
    ModeloInfo { id: "curriculo", nome: "Currículo", descricao: "Foto, contato, experiência, formação e habilidades em etiquetas." },
    ModeloInfo { id: "proposta", nome: "Proposta comercial", descricao: "Escopo, investimento com total e próximos passos." },
    ModeloInfo { id: "folheto", nome: "Folheto de evento", descricao: "Cartaz colorido com data, local e programação." },
    ModeloInfo { id: "convite", nome: "Convite", descricao: "Convite floral para casamento, aniversário ou festa." },
    ModeloInfo { id: "ata", nome: "Ata de reunião", descricao: "Participantes, pauta, decisões e responsáveis por ação." },
    ModeloInfo { id: "escolar", nome: "Trabalho escolar", descricao: "Capa ilustrada com nome, turma e professor, e o trabalho em seções." },
    ModeloInfo { id: "ciencias", nome: "Relatório de experimento", descricao: "Objetivo, hipótese, materiais, procedimento, resultados e conclusão." },
    ModeloInfo { id: "fichamento", nome: "Fichamento de leitura", descricao: "Dados da obra, ideia central, citações e sua opinião." },
    ModeloInfo { id: "academico", nome: "Trabalho acadêmico", descricao: "Capa, resumo, sumário e capítulos, no formato da ABNT." },
];

pub fn lista() -> &'static [ModeloInfo] {
    &MODELOS
}

// A paleta das ilustrações.
const MARINHO: u32 = 0x2e2d_4dff;
const CORAL: u32 = 0xf25c_54ff;
const VERDE: u32 = 0x1f8a_84ff;
const AMARELO: u32 = 0xf4b9_42ff;
const CREME: u32 = 0xfff6_e9ff;
const BRANCO: u32 = 0xffff_ffff;
const CINZA: u32 = 0x5b61_70ff;
const VERDE_CLARO: u32 = 0xe3f2_f0ff;
const CORAL_CLARO: u32 = 0xfde7_e4ff;
const AMARELO_CLARO: u32 = 0xfdf1_d6ff;

const LARGURA: f32 = 595.28;
const ALTURA: f32 = 841.89;

/// A letra de um texto do modelo: corpo, cor e bits (`NEGRITO`, `ITALICO`, `SERIFA`).
#[derive(Clone, Copy)]
struct Letra(f32, u32, u8);

impl Letra {
    const fn b(self) -> Self {
        Self(self.0, self.1, self.2 | NEGRITO)
    }
    const fn i(self) -> Self {
        Self(self.0, self.1, self.2 | ITALICO)
    }
    const fn serifa(self) -> Self {
        Self(self.0, self.1, self.2 | SERIFA)
    }
}

const fn l(tamanho: f32, cor: u32) -> Letra {
    Letra(tamanho, cor, 0)
}

const CORPO: Letra = l(10.5, CINZA);

/// `t` com a marcação `**negrito**`/`*itálico*`, na letra `le`; `\n` quebra a linha sem novo parágrafo.
fn trechos(t: &str, le: Letra) -> Vec<Trecho> {
    let mut v = analisar(&t.replace('\n', "\u{2028}"));
    for tr in &mut v {
        tr.estilo |= le.2;
        tr.fmt.tamanho = le.0;
        tr.fmt.cor = le.1;
    }
    v
}

fn base64(b: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = u32::from(c[0]) << 16 | u32::from(*c.get(1).unwrap_or(&0)) << 8 | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            s.push(if i <= c.len() { char::from(T[(n >> (18 - 6 * i) & 63) as usize]) } else { '=' });
        }
    }
    s
}

/// Uma página inteira de formas em SVG (as coordenadas são as da folha A4, em pontos).
fn svg(formas: &str) -> String {
    let s = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{LARGURA}" height="{ALTURA}" viewBox="0 0 {LARGURA} {ALTURA}">{formas}</svg>"#);
    format!("data:image/svg+xml;base64,{}", base64(s.as_bytes()))
}

fn png(bytes: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64(bytes))
}

const LIVROS: &[u8] = include_bytes!("../dados/modelos/livros.png");
const CIENCIAS: &[u8] = include_bytes!("../dados/modelos/ciencias.png");
const QUADRO: &[u8] = include_bytes!("../dados/modelos/quadro.png");
const LIVRO_ABERTO: &[u8] = include_bytes!("../dados/modelos/livro-aberto.png");
const FONES: &[u8] = include_bytes!("../dados/modelos/fones.png");
const FOGUETE: &[u8] = include_bytes!("../dados/modelos/foguete.png");
const REUNIAO: &[u8] = include_bytes!("../dados/modelos/reuniao.png");
const PESSOA: &[u8] = include_bytes!("../dados/modelos/pessoa.png");
const PLANTA: &[u8] = include_bytes!("../dados/modelos/planta.png");

fn sem_fio() -> Option<FioDeBorda> {
    Some(FioDeBorda { estilo: "nil".into(), ..FioDeBorda::default() })
}

fn fio(cor: u32) -> Option<FioDeBorda> {
    Some(FioDeBorda { estilo: "single".into(), espessura: 4, cor })
}

/// Acumula blocos; os métodos sem texto mudam o último bloco.
struct Montagem(Documento);

impl Montagem {
    fn novo(margens: [f32; 4]) -> Self {
        let mut d = Documento::vazio();
        let [margem_sup, margem_dir, margem_inf, margem_esq] = margens;
        d.pagina = ConfigPagina { margem_sup, margem_dir, margem_inf, margem_esq, ..ConfigPagina::a4() };
        Self(d)
    }
    fn titulo(mut self, nivel: u8, t: &str, le: Letra) -> Self {
        self.0.acrescentar(Conteudo::Titulo { nivel, texto: trechos(t, le) });
        self
    }
    fn par(mut self, t: &str, le: Letra) -> Self {
        self.0.acrescentar(Conteudo::Paragrafo { texto: trechos(t, le) });
        self
    }
    /// Parágrafo vazio de `altura` pontos: o espaço entre a capa e o resto.
    fn espaco(self, altura: f32) -> Self {
        self.par("", l(1.0, 0)).depois(altura - 1.0)
    }
    fn item(mut self, ordenado: bool, t: &str, le: Letra) -> Self {
        self.0.acrescentar(Conteudo::Item { ordenado, nivel: 0, texto: trechos(t, le) });
        self
    }
    fn bloco(mut self, c: Conteudo) -> Self {
        self.0.acrescentar(c);
        self
    }
    fn mudar(mut self, f: impl FnOnce(&mut crate::modelo::Bloco)) -> Self {
        if let Some(b) = self.0.blocos.last_mut() {
            f(b);
        }
        self
    }
    fn alinhado(self, a: Alinhamento) -> Self {
        self.mudar(|b| b.atrib.alinhamento = a)
    }
    fn antes(self, pt: f32) -> Self {
        self.mudar(|b| b.atrib.antes = Some(pt))
    }
    fn depois(self, pt: f32) -> Self {
        self.mudar(|b| b.atrib.depois = Some(pt))
    }
    fn entrelinha(self, x: f32) -> Self {
        self.mudar(|b| b.atrib.entrelinha = x)
    }
    fn recuo(self, esq: f32, dir: f32) -> Self {
        self.mudar(|b| {
            b.atrib.recuo_esq = esq;
            b.atrib.recuo_dir = dir;
        })
    }
    /// Caixa de destaque: fundo claro e a barra grossa à esquerda.
    fn chamada(self, fundo: u32, barra: u32) -> Self {
        self.mudar(|b| {
            b.atrib.fundo = fundo;
            b.atrib.borda = 1;
            b.atrib.cor_borda = barra;
        })
    }
    /// Tabela com as células já na letra delas; a primeira linha vai em `cabeca` (se `cabeca` ≠ 0)
    /// sobre o fundo `cabeca`, e as outras alternam branco e `zebra`. Sem os fios da grade.
    fn tabela(mut self, linhas: &[&[&str]], colunas: &[f32], le: Letra, cabeca: u32, zebra: u32) -> Self {
        let cabecalho = cabeca != 0;
        let conteudo = linhas.iter().enumerate()
            .map(|(i, l)| l.iter().map(|c| trechos(c, if i == 0 && cabecalho { Letra(le.0, BRANCO, NEGRITO) } else { le })).collect())
            .collect();
        self.0.acrescentar(Conteudo::Tabela { cabecalho, linhas: conteudo });
        let n = linhas.first().map_or(0, |l| l.len());
        self.mudar(|b| {
            let t = &mut b.tabela;
            t.colunas = colunas.to_vec();
            t.bordas = BordasDeTabela { cima: sem_fio(), esquerda: sem_fio(), baixo: sem_fio(), direita: sem_fio(), dentro_h: sem_fio(), dentro_v: sem_fio() };
            for (linha, _) in linhas.iter().enumerate() {
                let fundo = if linha == 0 && cabecalho { cabeca } else if (linha + usize::from(!cabecalho)) % 2 == 0 { zebra } else { 0 };
                for coluna in 0..n {
                    t.celulas.push(PropsCelula { linha, coluna, fundo, vertical: 1, margens: Some([5.0, 7.0, 5.0, 7.0]), ..PropsCelula::default() });
                }
            }
        })
    }
    /// Muda as células da tabela recém-posta: `f(linha, coluna, célula)`.
    fn celulas(self, f: impl Fn(usize, usize, &mut PropsCelula)) -> Self {
        self.mudar(|b| b.tabela.celulas.iter_mut().for_each(|c| f(c.linha, c.coluna, c)))
    }
    /// Fio fino embaixo de cada linha (a tabela de campos a preencher).
    fn linhas_de_campo(self, cor: u32) -> Self {
        self.mudar(|b| b.tabela.bordas.dentro_h = fio(cor)).mudar(|b| b.tabela.bordas.baixo = fio(cor))
    }
    /// Uma figura presa à página onde o último bloco começa; `atras` passa por baixo do texto.
    fn figura(mut self, origem: String, [x, y, largura, altura]: [f32; 4], atras: bool) -> Self {
        let bloco = self.0.blocos.last().map(|b| b.id);
        self.0.extras.decoracoes.push(Decoracao { origem, x, y, largura, altura, atras, bloco });
        self
    }
    /// A última figura sai em toda página (a capa, desenhada depois, a cobre na primeira).
    fn em_todas_as_paginas(mut self) -> Self {
        if let Some(d) = self.0.extras.decoracoes.last_mut() {
            d.bloco = None;
        }
        self
    }
    /// A ilustração PNG com a largura `w`, mantendo a proporção do arquivo.
    fn ilustracao(self, bytes: &[u8], x: f32, y: f32, w: f32) -> Self {
        // A largura e a altura do PNG ficam nos bytes 16..24 do cabeçalho IHDR.
        let dim = |i: usize| u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]) as f32;
        let h = w * dim(20) / dim(16);
        self.figura(png(bytes), [x, y, w, h], false)
    }
    fn fim(mut self, titulo: &str) -> Documento {
        self.0.meta.titulo = titulo.into();
        self.0
    }
}

/// O documento do modelo `id`; `None` se não existe.
pub fn criar(id: &str) -> Option<Documento> {
    use Alinhamento::{Centro, Direita, Justificado};
    let corpo_j = |m: Montagem, t: &str| m.par(t, CORPO).alinhado(Justificado).entrelinha(1.3).depois(8.0);
    let secao = |m: Montagem, t: &str, cor: u32| m.titulo(2, t, l(14.0, cor).b()).antes(16.0).depois(6.0);
    let d = match id {
        "em-branco" => Documento::vazio(),
        "carta" => {
            let formas = svg(&format!(
                r##"<rect width="{LARGURA}" height="132" fill="#2e2d4d"/>
                <circle cx="560" cy="18" r="96" fill="#f25c54"/><circle cx="472" cy="122" r="34" fill="#f4b942"/>
                <circle cx="530" cy="150" r="10" fill="#1f8a84"/>
                <rect y="806" width="{LARGURA}" height="36" fill="#2e2d4d"/><rect y="800" width="190" height="42" fill="#f25c54"/>
                <path d="M190 800 L222 842 L190 842 Z" fill="#f25c54"/>"##
            ));
            Montagem::novo([42.0, 72.0, 72.0, 64.0])
                .par("Mariana Souza", l(28.0, BRANCO).b()).depois(2.0)
                .figura(formas, [0.0, 0.0, LARGURA, ALTURA], true)
                .par("Designer de Produto", l(12.0, AMARELO)).depois(6.0)
                .par("(11) 90000-0000  ·  mariana@email.com  ·  São Paulo, SP", l(9.0, 0xd9dce8ff)).depois(64.0)
                .par("São Paulo, 28 de outubro de 2026", l(10.0, CORAL).b()).alinhado(Direita).depois(22.0)
                .par("À **Empresa Exemplo Ltda.**\nDepartamento de Compras\nRua das Flores, 123 — São Paulo, SP", l(10.5, MARINHO)).entrelinha(1.3).depois(22.0)
                .par("**Assunto:** proposta de fornecimento", l(10.5, MARINHO)).depois(18.0)
                .par("Prezados senhores,", CORPO).depois(10.0)
                .apply(|m| corpo_j(m, "Escreva aqui o motivo da carta. Diga em poucas linhas o que você precisa ou oferece, e o que espera como resposta. Uma carta boa cabe em uma página e vai direto ao ponto."))
                .apply(|m| corpo_j(m, "No segundo parágrafo, dê os detalhes: prazos, valores, condições ou documentos anexos. Se houver mais de um assunto, um parágrafo para cada."))
                .apply(|m| corpo_j(m, "Fico à disposição para conversar e agradeço desde já a atenção."))
                .par("Atenciosamente,", CORPO).antes(14.0).depois(36.0)
                .par("Mariana Souza", l(13.0, MARINHO).b()).depois(0.0)
                .par("Designer de Produto", l(10.0, CORAL))
                .fim("Carta")
        }
        "relatorio" => {
            let capa = svg(
                r##"<rect width="595.28" height="560" fill="#2e2d4d"/>
                <circle cx="0" cy="0" r="104" fill="#f25c54"/><circle cx="0" cy="0" r="66" fill="#2e2d4d"/>
                <circle cx="560" cy="560" r="190" fill="#1f8a84"/>
                <g fill="#f4b942"><circle cx="420" cy="70" r="4"/><circle cx="440" cy="70" r="4"/><circle cx="460" cy="70" r="4"/><circle cx="480" cy="70" r="4"/>
                <circle cx="420" cy="90" r="4"/><circle cx="440" cy="90" r="4"/><circle cx="460" cy="90" r="4"/><circle cx="480" cy="90" r="4"/>
                <circle cx="420" cy="110" r="4"/><circle cx="440" cy="110" r="4"/><circle cx="460" cy="110" r="4"/><circle cx="480" cy="110" r="4"/></g>
                <g transform="translate(320 300)"><rect x="0" y="150" width="34" height="60" rx="4" fill="#f4b942"/>
                <rect x="46" y="110" width="34" height="100" rx="4" fill="#f25c54"/><rect x="92" y="70" width="34" height="140" rx="4" fill="#fff6e9"/>
                <rect x="138" y="20" width="34" height="190" rx="4" fill="#f4b942"/>
                <path d="M-10 160 L63 100 L109 120 L170 -10" stroke="#fff6e9" stroke-width="4" fill="none" stroke-linecap="round" stroke-linejoin="round"/>
                <path d="M150 -14 L176 -18 L172 8" stroke="#fff6e9" stroke-width="4" fill="none" stroke-linecap="round" stroke-linejoin="round"/></g>
                <rect y="560" width="595.28" height="281.89" fill="#fff6e9"/><rect x="64" y="600" width="60" height="6" fill="#f25c54"/>"##,
            );
            let faixa = svg(r##"<rect width="10" height="841.89" fill="#1f8a84"/><rect y="0" width="10" height="120" fill="#f25c54"/>"##);
            Montagem::novo([72.0, 64.0, 64.0, 64.0])
                .par("RELATÓRIO ANUAL · 2026", l(11.0, AMARELO).b()).antes(60.0).depois(14.0)
                .figura(faixa, [0.0, 0.0, LARGURA, ALTURA], true)
                .em_todas_as_paginas()
                .figura(capa, [0.0, 0.0, LARGURA, ALTURA], true)
                .par("Relatório de\natividades", l(42.0, BRANCO).b()).entrelinha(1.05).recuo(0.0, 170.0).depois(18.0)
                .par("O ano em números, o que aprendemos e o plano para o próximo período.", l(13.0, 0xd9dce8ff)).recuo(0.0, 280.0).entrelinha(1.3)
                .espaco(300.0)
                .par("Preparado por", l(10.0, CORAL).b()).depois(2.0)
                .par("Equipe de Operações", l(16.0, MARINHO).b()).depois(2.0)
                .par("Outubro de 2026", l(11.0, CINZA))
                .bloco(Conteudo::QuebraDePagina)
                .bloco(Conteudo::sumario(2))
                .titulo(1, "Resumo", l(22.0, MARINHO).b())
                .apply(|m| corpo_j(m, "Em um parágrafo, o que foi feito, o resultado principal e a recomendação. Quem ler só esta página precisa sair sabendo o essencial."))
                .par("**Destaque do ano:** a satisfação dos clientes chegou a 93%, três pontos acima da meta, com o prazo médio de atendimento caindo para 2,4 dias.", l(10.5, MARINHO)).chamada(VERDE_CLARO, VERDE).entrelinha(1.3).antes(6.0).depois(14.0)
                .tabela(&[&["1.348\natendimentos", "2,4 dias\nprazo médio", "93%\nsatisfação"]], &[155.0, 155.0, 155.0], l(12.0, BRANCO).b(), 0, 0)
                .celulas(|_, c, p| {
                    p.fundo = [VERDE, CORAL, MARINHO][c];
                    p.alinhamento = Centro;
                    p.margens = Some([12.0, 8.0, 12.0, 8.0]);
                })
                .titulo(1, "Resultados", l(22.0, MARINHO).b()).antes(24.0)
                .par("Os números do período, comparados com a meta:", CORPO).depois(8.0)
                .tabela(&[&["Indicador", "Meta", "Realizado", "Situação"], &["Atendimentos", "1.200", "1.348", "Acima"], &["Prazo médio", "3 dias", "2,4 dias", "Acima"], &["Satisfação", "90%", "93%", "Acima"], &["Retrabalho", "5%", "6,1%", "Abaixo"]], &[170.0, 90.0, 100.0, 107.0], l(10.0, MARINHO), MARINHO, VERDE_CLARO)
                .titulo(2, "Destaques", l(14.0, CORAL).b()).antes(18.0)
                .item(false, "Primeiro destaque do período, com o impacto dele.", CORPO)
                .item(false, "Segundo destaque: o que mudou e por quê.", CORPO)
                .titulo(1, "Próximos passos", l(22.0, MARINHO).b()).antes(24.0)
                .item(true, "Ação, responsável e prazo.", CORPO)
                .item(true, "Outra ação, com o resultado esperado.", CORPO)
                .fim("Relatório de atividades")
        }
        "curriculo" => {
            let formas = svg(
                r##"<rect width="595.28" height="232" fill="#fff6e9"/>
                <circle cx="134" cy="118" r="88" fill="#1f8a84"/><circle cx="215" cy="46" r="16" fill="#f4b942"/>
                <circle cx="600" cy="0" r="120" fill="#f25c54" opacity="0.9"/><circle cx="520" cy="210" r="8" fill="#1f8a84"/>
                <rect x="0" y="232" width="595.28" height="5" fill="#f25c54"/>"##,
            );
            let etiquetas = |m: Montagem, linhas: &[&[&str]]| {
                m.tabela(linhas, &[155.0, 155.0, 155.0], l(9.5, MARINHO).b(), 0, 0).celulas(|_, _, p| {
                    p.fundo = VERDE_CLARO;
                    p.alinhamento = Centro;
                })
            };
            Montagem::novo([64.0, 56.0, 56.0, 56.0])
                .par("Mariana Souza", l(32.0, MARINHO).b()).recuo(190.0, 0.0).depois(2.0)
                .figura(formas, [0.0, 0.0, LARGURA, ALTURA], true)
                .ilustracao(PESSOA, 58.0, 36.0, 152.0)
                .par("DESIGNER DE PRODUTO", l(12.0, CORAL).b()).recuo(190.0, 0.0).depois(14.0)
                .par("(11) 90000-0000\nmariana@email.com\nlinkedin.com/in/mariana · São Paulo, SP", l(9.5, CINZA)).recuo(190.0, 0.0).entrelinha(1.35)
                .espaco(62.0)
                .apply(|m| secao(m, "Sobre mim", VERDE))
                .apply(|m| corpo_j(m, "Designer com 7 anos de experiência em produtos digitais. Gosto de transformar problemas confusos em telas simples, testando cedo com quem vai usar."))
                .apply(|m| secao(m, "Experiência", VERDE))
                .par("**Designer de Produto Sênior** — Empresa Exemplo", l(11.0, MARINHO)).depois(0.0)
                .par("2023 – atual", l(9.5, CORAL).b()).depois(4.0)
                .item(false, "Redesenhou o cadastro e reduziu o abandono em 30%.", CORPO)
                .item(false, "Criou o sistema de design usado por quatro equipes.", CORPO)
                .par("**Designer de Interface** — Agência Exemplo", l(11.0, MARINHO)).antes(10.0).depois(0.0)
                .par("2020 – 2023", l(9.5, CORAL).b()).depois(4.0)
                .item(false, "Projetos para varejo e saúde, do protótipo à entrega.", CORPO)
                .apply(|m| secao(m, "Formação", VERDE))
                .par("**Design Digital** — Universidade Exemplo · 2019", l(10.5, MARINHO))
                .apply(|m| secao(m, "Habilidades", VERDE))
                .apply(|m| etiquetas(m, &[&["Pesquisa com usuários", "Protótipos", "Sistemas de design"], &["Figma", "Acessibilidade", "Inglês avançado"]]))
                .fim("Currículo")
        }
        "proposta" => {
            let formas = svg(
                r##"<path d="M300 0 H595.28 V330 Z" fill="#fde7e4"/><path d="M420 0 H595.28 V190 Z" fill="#f25c54"/>
                <circle cx="80" cy="790" r="110" fill="#e3f2f0"/><rect x="64" y="52" width="48" height="6" fill="#f25c54"/>"##,
            );
            Montagem::novo([80.0, 64.0, 64.0, 64.0])
                .par("PROPOSTA COMERCIAL", l(11.0, CORAL).b()).depois(10.0)
                .figura(formas, [0.0, 0.0, LARGURA, ALTURA], true)
                .ilustracao(FOGUETE, 372.0, 118.0, 170.0)
                .par("Vamos tirar\nseu projeto\ndo papel", l(36.0, MARINHO).b()).entrelinha(1.05).recuo(0.0, 200.0).depois(14.0)
                .par("Para **Empresa Exemplo Ltda.** · 28 de outubro de 2026", l(11.0, CINZA)).depois(70.0)
                .titulo(1, "Sobre nós", l(18.0, MARINHO).b())
                .apply(|m| corpo_j(m, "Em duas ou três linhas, quem é a sua empresa, há quanto tempo atua e por que ela é a escolha certa para este trabalho."))
                .titulo(1, "O que vamos entregar", l(18.0, MARINHO).b()).antes(14.0)
                .item(false, "**Diagnóstico** do processo atual, com entrevistas.", CORPO)
                .item(false, "**Projeto** da solução, aprovado com a sua equipe.", CORPO)
                .item(false, "**Implantação** e treinamento, com 30 dias de suporte.", CORPO)
                .titulo(1, "Investimento", l(18.0, MARINHO).b()).antes(14.0).depois(8.0)
                .tabela(&[&["Etapa", "Prazo", "Valor"], &["Diagnóstico", "2 semanas", "R$ 4.000"], &["Projeto", "4 semanas", "R$ 9.500"], &["Implantação", "3 semanas", "R$ 6.500"], &["Total", "9 semanas", "R$ 20.000"]], &[200.0, 130.0, 137.0], l(10.5, MARINHO), MARINHO, CORAL_CLARO)
                .celulas(|linha, coluna, p| {
                    if linha == 4 {
                        p.fundo = AMARELO;
                    }
                    if coluna == 2 {
                        p.alinhamento = Direita;
                    }
                })
                .par("Proposta válida por 30 dias. Pagamento em três parcelas, uma no início de cada etapa.", l(9.5, CINZA).i()).antes(10.0)
                .fim("Proposta comercial")
        }
        "folheto" => {
            let formas = svg(
                r##"<rect width="595.28" height="841.89" fill="#2e2d4d"/>
                <path d="M330 0 H595.28 V265 Z" fill="#f25c54"/><circle cx="0" cy="841.89" r="200" fill="#1f8a84"/>
                <circle cx="0" cy="841.89" r="130" fill="#2e2d4d"/><circle cx="0" cy="841.89" r="90" fill="#f4b942"/>
                <circle cx="520" cy="780" r="26" fill="#f4b942"/><circle cx="470" cy="815" r="9" fill="#f25c54"/>
                <g fill="#1f8a84"><circle cx="64" cy="120" r="3"/><circle cx="80" cy="120" r="3"/><circle cx="96" cy="120" r="3"/>
                <circle cx="64" cy="136" r="3"/><circle cx="80" cy="136" r="3"/><circle cx="96" cy="136" r="3"/></g>"##,
            );
            Montagem::novo([64.0, 64.0, 48.0, 64.0])
                .par("AO VIVO · ENTRADA GRATUITA", l(11.0, AMARELO).b()).antes(110.0).depois(10.0)
                .figura(formas, [0.0, 0.0, LARGURA, ALTURA], true)
                .ilustracao(FONES, 318.0, 300.0, 230.0)
                .par("Noite do\nPodcast", l(56.0, BRANCO).b()).entrelinha(1.0).depois(46.0)
                .par("15 de novembro, 19h", l(20.0, CORAL).b()).depois(4.0)
                .par("Auditório Central · Rua das Flores, 123", l(12.0, CREME)).depois(26.0)
                .par("Três conversas gravadas na frente da plateia, com perguntas do público e música ao vivo no intervalo.", l(12.0, 0xd9dce8ff)).recuo(0.0, 270.0).entrelinha(1.35).depois(24.0)
                .item(false, "19h — Abertura e bate-papo", l(11.0, BRANCO))
                .item(false, "20h — Entrevista especial", l(11.0, BRANCO))
                .item(false, "21h — Show de encerramento", l(11.0, BRANCO))
                .par("Inscreva-se: **seusite.com.br/podcast**", l(12.0, AMARELO)).antes(30.0)
                .fim("Folheto de evento")
        }
        "convite" => {
            let formas = svg(
                r##"<rect width="595.28" height="841.89" fill="#fff6e9"/>
                <rect x="28" y="28" width="539.28" height="785.89" fill="none" stroke="#f4b942" stroke-width="1.5"/>
                <rect x="36" y="36" width="523.28" height="769.89" fill="none" stroke="#f4b942" stroke-width="0.6"/>
                <path d="M247 470 H347" stroke="#f25c54" stroke-width="1"/><circle cx="297.6" cy="470" r="3" fill="#f25c54"/>"##,
            );
            let flor = png(PLANTA);
            let (w, h) = (210.0, 210.0 * 424.0 / 520.0);
            Montagem::novo([72.0, 80.0, 72.0, 80.0])
                .par("com a bênção de seus pais", l(11.0, CINZA).serifa().i()).antes(170.0).alinhado(Centro).depois(26.0)
                .figura(formas, [0.0, 0.0, LARGURA, ALTURA], true)
                .figura(flor.clone(), [-24.0, -16.0, w, h], true)
                .figura(flor, [LARGURA - w + 24.0, ALTURA - h + 16.0, w, h], true)
                .par("Ana & Pedro", l(50.0, MARINHO).serifa().i()).alinhado(Centro).depois(18.0)
                .par("convidam para a celebração do seu casamento", l(13.0, CINZA).serifa()).alinhado(Centro).depois(58.0)
                .par("SÁBADO · 12 DE DEZEMBRO DE 2026 · 16H", l(12.0, CORAL).b()).alinhado(Centro).depois(10.0)
                .par("Espaço Jardim das Flores\nEstrada do Sol, 450 — Campinas, SP", l(12.0, MARINHO).serifa()).alinhado(Centro).entrelinha(1.4).depois(40.0)
                .par("Confirme sua presença até 20 de novembro\n(11) 90000-0000", l(10.0, CINZA).serifa().i()).alinhado(Centro).entrelinha(1.4)
                .fim("Convite")
        }
        "ata" => {
            let formas = svg(r##"<rect width="595.28" height="10" fill="#1f8a84"/><rect width="160" height="10" fill="#f25c54"/>"##);
            Montagem::novo([56.0, 64.0, 64.0, 64.0])
                .par("PROJETO AURORA · REUNIÃO SEMANAL", l(10.0, CORAL).b()).antes(28.0).depois(6.0)
                .figura(formas, [0.0, 0.0, LARGURA, ALTURA], true)
                .ilustracao(REUNIAO, 366.0, 34.0, 170.0)
                .par("Ata de\nreunião", l(34.0, MARINHO).b()).entrelinha(1.0).depois(34.0)
                .tabela(&[&["Data", "28/10/2026, 14h"], &["Local", "Sala 2 / videochamada"], &["Participantes", "Ana, Bruno, Carla, Diego"], &["Redação", "Seu nome"]], &[120.0, 347.0], l(10.5, MARINHO), 0, 0)
                .celulas(|_, c, p| p.fundo = if c == 0 { VERDE_CLARO } else { 0 })
                .linhas_de_campo(0xd5e8e5ff)
                .titulo(1, "Pauta", l(16.0, VERDE).b()).antes(20.0)
                .item(true, "Situação do cronograma.", CORPO)
                .item(true, "Orçamento do próximo trimestre.", CORPO)
                .item(true, "Assuntos gerais.", CORPO)
                .titulo(1, "Discussão e decisões", l(16.0, VERDE).b()).antes(16.0)
                .apply(|m| corpo_j(m, "O que foi conversado em cada item da pauta, com as opiniões principais."))
                .par("**Decidido:** a entrega da fase 2 passa para 15/11, com revisão do escopo na próxima reunião.", l(10.5, MARINHO)).chamada(CORAL_CLARO, CORAL).entrelinha(1.3).depois(8.0)
                .titulo(1, "Próximos passos", l(16.0, VERDE).b()).antes(16.0).depois(8.0)
                .tabela(&[&["Ação", "Responsável", "Prazo"], &["Enviar a proposta revisada", "Ana", "04/11"], &["Revisar o orçamento", "Bruno", "07/11"], &["Marcar a demonstração", "Carla", "10/11"]], &[247.0, 120.0, 100.0], l(10.0, MARINHO), VERDE, VERDE_CLARO)
                .fim("Ata de reunião")
        }
        "escolar" => {
            let formas = svg(
                r##"<rect width="595.28" height="841.89" fill="#fff6e9"/>
                <path d="M0 0 H250 C230 90 140 150 0 170 Z" fill="#f4b942"/><circle cx="60" cy="40" r="26" fill="#f25c54"/>
                <path d="M595.28 841.89 V600 C520 640 470 760 380 841.89 Z" fill="#1f8a84"/>
                <circle cx="530" cy="80" r="14" fill="#1f8a84"/><circle cx="500" cy="120" r="6" fill="#f25c54"/>
                <circle cx="80" cy="760" r="10" fill="#f25c54"/><circle cx="112" cy="790" r="5" fill="#f4b942"/>
                <g fill="none" stroke="#f25c54" stroke-width="3" stroke-linecap="round"><path d="M220 238 q15 -12 30 0 t30 0 t30 0 t30 0 t30 0"/></g>"##,
            );
            let pagina = svg(r##"<path d="M595.28 0 V60 C560 50 540 20 535 0 Z" fill="#f4b942"/>"##);
            Montagem::novo([72.0, 72.0, 72.0, 72.0])
                .par("Trabalho de Ciências", l(36.0, MARINHO).b()).antes(84.0).alinhado(Centro).depois(36.0)
                .figura(formas, [0.0, 0.0, LARGURA, ALTURA], true)
                .ilustracao(QUADRO, 128.0, 268.0, 340.0)
                .par("O ciclo da água", l(18.0, CORAL).b()).alinhado(Centro)
                .espaco(290.0)
                .tabela(&[&["Nome", "Seu nome completo"], &["Turma", "7º ano B"], &["Professor(a)", "Nome do professor"], &["Data", "28/10/2026"]], &[120.0, 260.0], l(12.0, MARINHO), 0, 0)
                .celulas(|_, c, p| if c == 0 { p.alinhamento = Direita })
                .linhas_de_campo(0xe6c98aff)
                .bloco(Conteudo::QuebraDePagina)
                .titulo(1, "Introdução", l(20.0, MARINHO).b())
                .figura(pagina, [0.0, 0.0, LARGURA, ALTURA], true)
                .apply(|m| corpo_j(m, "Conte do que o trabalho trata e por que o tema é importante. Duas ou três frases bastam."))
                .titulo(1, "Desenvolvimento", l(20.0, MARINHO).b()).antes(18.0)
                .titulo(2, "Evaporação", l(14.0, CORAL).b())
                .apply(|m| corpo_j(m, "Explique cada parte com as suas palavras. Use exemplos do dia a dia."))
                .titulo(2, "Condensação e precipitação", l(14.0, CORAL).b())
                .apply(|m| corpo_j(m, "Continue a explicação. Se fizer um desenho ou experimento, descreva aqui."))
                .par("**Você sabia?** Uma gota de água pode levar milhares de anos para voltar ao mar.", l(10.5, MARINHO)).chamada(AMARELO_CLARO, AMARELO).entrelinha(1.3).antes(6.0)
                .titulo(1, "Conclusão", l(20.0, MARINHO).b()).antes(18.0)
                .apply(|m| corpo_j(m, "O que você aprendeu com o trabalho."))
                .titulo(1, "Referências", l(20.0, MARINHO).b()).antes(18.0)
                .item(false, "Livro, site ou vídeo que você consultou.", CORPO)
                .ilustracao(LIVROS, 430.0, 640.0, 110.0)
                .fim("Trabalho escolar")
        }
        "ciencias" => {
            let formas = svg(
                r##"<rect width="595.28" height="200" fill="#e3f2f0"/><circle cx="-10" cy="200" r="60" fill="#1f8a84"/>
                <circle cx="330" cy="30" r="8" fill="#f4b942"/><circle cx="300" cy="170" r="5" fill="#f25c54"/>"##,
            );
            Montagem::novo([60.0, 64.0, 64.0, 64.0])
                .par("RELATÓRIO DE EXPERIMENTO", l(10.0, VERDE).b()).antes(14.0).depois(6.0)
                .figura(formas, [0.0, 0.0, LARGURA, ALTURA], true)
                .ilustracao(CIENCIAS, 340.0, 22.0, 210.0)
                .par("Separação\nde misturas", l(32.0, MARINHO).b()).entrelinha(1.0).recuo(0.0, 230.0).depois(8.0)
                .par("Laboratório de Química · Grupo 3", l(11.0, CINZA)).depois(42.0)
                .tabela(&[&["Alunos", "Ana, Bruno e Carla"], &["Data", "28/10/2026"], &["Professor(a)", "Nome do professor"]], &[110.0, 357.0], l(10.5, MARINHO), 0, 0)
                .celulas(|_, c, p| p.fundo = if c == 0 { VERDE_CLARO } else { 0 })
                .linhas_de_campo(0xd5e8e5ff)
                .apply(|m| secao(m, "Objetivo", VERDE))
                .apply(|m| corpo_j(m, "O que o experimento quer descobrir ou mostrar, em uma frase."))
                .apply(|m| secao(m, "Hipótese", VERDE))
                .par("*Se* filtrarmos a mistura de água e areia, *então* a areia ficará retida no papel.", l(10.5, MARINHO)).chamada(AMARELO_CLARO, AMARELO).entrelinha(1.3).depois(8.0)
                .apply(|m| secao(m, "Materiais", VERDE))
                .tabela(&[&["• Béquer de 250 mL", "• Funil e papel-filtro", "• Água e areia"]], &[155.0, 155.0, 155.0], l(10.0, MARINHO), 0, 0)
                .apply(|m| secao(m, "Procedimento", VERDE))
                .item(true, "Misture duas colheres de areia em 200 mL de água.", CORPO)
                .item(true, "Dobre o papel-filtro, encaixe no funil e despeje a mistura.", CORPO)
                .item(true, "Anote o que fica no papel e o que passa para o béquer.", CORPO)
                .apply(|m| secao(m, "Resultados", VERDE))
                .tabela(&[&["Etapa", "Observação", "Tempo"], &["Mistura", "Água turva", "1 min"], &["Filtragem", "Areia retida no papel", "5 min"], &["Final", "Água quase limpa", "—"]], &[120.0, 247.0, 100.0], l(10.0, MARINHO), VERDE, VERDE_CLARO)
                .apply(|m| secao(m, "Conclusão", VERDE))
                .apply(|m| corpo_j(m, "A hipótese foi confirmada? O que daria para melhorar se o experimento fosse repetido?"))
                .fim("Relatório de experimento")
        }
        "fichamento" => {
            let formas = svg(r##"<rect width="16" height="841.89" fill="#f25c54"/><rect x="16" width="6" height="841.89" fill="#f4b942"/>"##);
            let citacao = |m: Montagem, t: &str, p: &str| {
                m.par(t, l(12.0, MARINHO).serifa().i()).chamada(CORAL_CLARO, CORAL).entrelinha(1.35).antes(6.0).depois(2.0)
                    .par(p, l(9.0, CORAL).b()).alinhado(Direita).depois(10.0)
            };
            Montagem::novo([64.0, 64.0, 64.0, 72.0])
                .par("FICHAMENTO DE LEITURA", l(10.0, CORAL).b()).antes(8.0).depois(6.0)
                .figura(formas, [0.0, 0.0, LARGURA, ALTURA], true)
                .ilustracao(LIVRO_ABERTO, 360.0, 40.0, 180.0)
                .par("Título\nda obra", l(34.0, MARINHO).b().serifa()).entrelinha(1.0).recuo(0.0, 200.0).depois(40.0)
                .tabela(&[&["Autor", "SOBRENOME, Nome"], &["Editora e ano", "Editora, 2024"], &["Páginas lidas", "1 a 120"], &["Leitor", "Seu nome"]], &[120.0, 339.0], l(10.5, MARINHO), 0, 0)
                .celulas(|_, c, p| p.fundo = if c == 0 { AMARELO_CLARO } else { 0 })
                .linhas_de_campo(0xf0dcb0ff)
                .apply(|m| secao(m, "Ideia central", CORAL))
                .apply(|m| corpo_j(m, "Em três ou quatro linhas, a tese do autor: o que ele defende e com que argumentos."))
                .apply(|m| secao(m, "Citações", CORAL))
                .apply(|m| citacao(m, "“Copie aqui o trecho que mais chamou a sua atenção, entre aspas e com a página.”", "p. 42"))
                .apply(|m| citacao(m, "“Uma segunda citação, que mostre outro lado do argumento.”", "p. 87"))
                .apply(|m| secao(m, "Comentário pessoal", CORAL))
                .apply(|m| corpo_j(m, "Você concorda com o autor? O texto se liga a outras leituras ou a algo da sua vida?"))
                .apply(|m| secao(m, "Palavras-chave", CORAL))
                .tabela(&[&["conceito", "autor", "tema"]], &[100.0, 100.0, 100.0], l(9.5, MARINHO).b(), 0, 0)
                .celulas(|_, _, p| {
                    p.fundo = AMARELO_CLARO;
                    p.alinhamento = Centro;
                })
                .fim("Fichamento de leitura")
        }
        "academico" => {
            // ABNT: margens de 3 cm em cima e à esquerda, 2 cm à direita e embaixo; letra 12 e entrelinha 1,5.
            let s = l(12.0, 0x1a1a_1aff).serifa();
            let txt = |m: Montagem, t: &str| m.par(t, s).alinhado(Justificado).entrelinha(1.5).mudar(|b| b.atrib.primeira = 35.4);
            let cap = |m: Montagem, t: &str| m.titulo(1, t, s.b()).antes(0.0).depois(18.0);
            Montagem::novo([85.0, 57.0, 57.0, 85.0])
                .par("UNIVERSIDADE EXEMPLO\nCURSO DE EXEMPLO", s.b()).alinhado(Centro).entrelinha(1.5)
                .par("NOME DO AUTOR", s.b()).alinhado(Centro).antes(120.0)
                .par("TÍTULO DO TRABALHO:\nsubtítulo", s.b()).alinhado(Centro).antes(150.0).entrelinha(1.5)
                .par("Cidade\n2026", s.b()).alinhado(Centro).antes(250.0).entrelinha(1.5)
                .bloco(Conteudo::QuebraDePagina)
                .par("RESUMO", s.b()).alinhado(Centro).depois(18.0)
                .par("Texto corrido de 150 a 500 palavras, em parágrafo único: objetivo, método, resultados e conclusões.", s).alinhado(Justificado).entrelinha(1.0)
                .par("**Palavras-chave:** palavra; outra palavra; mais uma.", s).antes(12.0)
                .bloco(Conteudo::QuebraDePagina)
                .par("SUMÁRIO", s.b()).alinhado(Centro).depois(18.0)
                .bloco(Conteudo::sumario(3))
                .bloco(Conteudo::QuebraDePagina)
                .apply(|m| cap(m, "1 INTRODUÇÃO"))
                .apply(|m| txt(m, "Apresente o tema, o problema e os objetivos do trabalho."))
                .apply(|m| cap(m, "2 DESENVOLVIMENTO"))
                .titulo(2, "2.1 Fundamentação teórica", s.b())
                .apply(|m| txt(m, "O que os autores já disseram sobre o tema."))
                .apply(|m| cap(m, "3 CONCLUSÃO"))
                .apply(|m| txt(m, "Retome os objetivos e diga o que foi alcançado."))
                .par("REFERÊNCIAS", s.b()).alinhado(Centro).antes(18.0).depois(18.0)
                .par("SOBRENOME, Nome. **Título da obra**. Cidade: Editora, ano.", s)
                .fim("Trabalho acadêmico")
        }
        _ => return None,
    };
    Some(d)
}

impl Montagem {
    fn apply(self, f: impl FnOnce(Self) -> Self) -> Self {
        f(self)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn todo_modelo_da_lista_cria_um_documento_que_pagina() {
        for m in lista() {
            let d = criar(m.id).unwrap_or_else(|| panic!("{} não cria", m.id));
            let s = crate::sessao::Sessao::com_documento(d);
            assert!(s.total_de_paginas() >= 1, "{}", m.id);
            assert!(s.quadro(0, u64::MAX).is_ok(), "{}: a miniatura sai da primeira página", m.id);
        }
        assert!(criar("nao-existe").is_none());
    }

    #[test]
    fn base64_e_as_capas_de_uma_pagina() {
        assert_eq!((base64(b"Man"), base64(b"Ma"), base64(b"M")), ("TWFu".into(), "TWE=".into(), "TQ==".into()));
        // A capa não pode transbordar: o texto dela fica na página das formas.
        for id in ["carta", "curriculo", "proposta", "folheto", "convite", "fichamento", "ciencias", "ata"] {
            let s = crate::sessao::Sessao::com_documento(criar(id).unwrap());
            assert_eq!(s.total_de_paginas(), 1, "{id} cabe em uma página");
        }
    }
}
