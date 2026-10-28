//! Modo dividido: o código LaTeX ou Markdown à esquerda das páginas, os dois editáveis.
//!
//! A interface só guarda o texto e repassa: o código digitado vai ao motor depois de uma pausa
//! (`dividido::reconciliar` relê e `Sessao::trocar_blocos` troca só os blocos que mudaram);
//! quando o documento muda pela página, o código é gerado de novo pelo motor, com as linhas de
//! cada bloco. Enquanto a pessoa digita no código ele não é reescrito.
use featherdocs_core::modelo::Documento;
use featherdocs_io::dividido::{self, Fonte};
use featherdocs_io::exportar::{self, LinhasDoBloco, Mapa};
use featherdocs_io::realce::{self, Tipo};
use pelicano::{Color, Content, Font};
use std::ops::Range;
use std::time::{Duration, Instant};

/// A pausa na digitação antes de mandar o código ao motor.
pub const PAUSA: Duration = Duration::from_millis(300);

/// A fonte monoespaçada do sistema (DejaVu Sans Mono, Consolas…). A Liberation Mono embutida
/// tem maiúsculas baixas demais nesse corpo: na tela o código parecia todo em minúsculas.
pub const FONTE_MONO: Font = Font::MONO;
pub const TAMANHO: f32 = 13.5;
pub const PADDING: f32 = 10.;
/// A altura de uma linha do editor (1,3 × o tamanho, a do Iced); o editor do pelicano a recebe em
/// `line_height` e a usa nos números de linha que ele mesmo desenha.
pub const ALTURA_DA_LINHA: f32 = TAMANHO * 1.3;

pub struct PainelCodigo {
    pub fonte: Fonte,
    pub conteudo: Content,
    /// As linhas `[de, ate)` de cada bloco no texto do editor.
    pub blocos: Vec<LinhasDoBloco>,
    /// A versão da sessão que o texto do editor representa.
    pub versao: u64,
    /// Instante da última edição ainda não aplicada (pendente).
    pub ultima_edicao: Option<Instant>,
    /// O bloco cuja linha o cursor do código mostra agora.
    pub bloco_marcado: Option<u64>,
}

impl PainelCodigo {
    pub fn novo(fonte: Fonte, mapa: Mapa, versao: u64) -> Self {
        Self {
            fonte,
            conteudo: Content::with_text(&mapa.texto),
            blocos: mapa.blocos,
            versao,
            ultima_edicao: None,
            bloco_marcado: None,
        }
    }

    pub fn pendente(&self) -> bool {
        self.ultima_edicao.is_some()
    }

    /// A pausa terminou: hora de aplicar o código.
    pub fn maduro(&self, agora: Instant) -> bool {
        self.ultima_edicao.is_some_and(|t| agora.duration_since(t) >= PAUSA)
    }

    /// Troca o texto inteiro do editor (o documento mudou pela página).
    pub fn recarregar(&mut self, mapa: Mapa, versao: u64) {
        if self.conteudo.text() != mapa.texto {
            self.conteudo = Content::with_text(&mapa.texto);
            self.bloco_marcado = None;
        }
        self.blocos = mapa.blocos;
        self.versao = versao;
    }

    /// O bloco da linha `linha` do código (o último que começa nela ou antes).
    pub fn bloco_da_linha(&self, linha: usize) -> Option<u64> {
        self.blocos
            .iter()
            .filter(|b| b.de <= linha)
            .max_by_key(|b| b.de)
            .map(|b| b.id)
    }

    /// Leva o cursor do código ao começo do bloco `id`; o editor rola até ele.
    pub fn ir_ao_bloco(&mut self, id: u64) -> bool {
        if self.bloco_marcado == Some(id) {
            return false;
        }
        let Some(linha) = self.blocos.iter().find(|b| b.id == id).map(|b| b.de) else {
            return false;
        };
        self.ir_a_linha(linha);
        self.bloco_marcado = Some(id);
        true
    }

    /// Leva o cursor do código à linha `linha` (contando de 0); o editor rola até ela.
    // shortcut: o editor do pelicano só rola até o cursor aparecer (a linha fica na beirada, sem as 12
    // linhas de contexto que o Iced deixava embaixo); pedir ao pelicano um "rolar com margem" se fizer falta.
    pub fn ir_a_linha(&mut self, linha: usize) {
        self.conteudo.go_to_line(linha + 1);
    }

    /// A linha do cursor do código, contando de 0 (o `cursor_position().0` do Iced).
    pub fn linha_do_cursor(&self) -> usize {
        self.conteudo.cursor().0 - 1
    }
}

/// O código do documento na `fonte` pedida. Se `original` é um texto da pessoa (o `.tex` aberto
/// ou o último código aplicado) e ainda descreve este documento, ele é mantido na forma dela;
/// senão o motor escreve o código na forma dele.
pub fn gerar(doc: &Documento, fonte: Fonte, original: Option<&(Fonte, String)>) -> Mapa {
    if let Some((f, texto)) = original {
        if *f == fonte && descreve(doc, fonte, texto) {
            return exportar::mapa_de_texto(texto, doc, fonte);
        }
    }
    match fonte {
        Fonte::Latex => exportar::latex_com_mapa(doc),
        Fonte::Markdown => exportar::markdown_com_mapa(doc),
    }
}

/// O texto, relido, daria exatamente este documento (nada a trocar)?
fn descreve(doc: &Documento, fonte: Fonte, texto: &str) -> bool {
    let r = dividido::reconciliar(doc, fonte, texto);
    r.meta.is_none()
        && r.pagina.is_none_or(|p| dividido::mesma_pagina(&p, &doc.pagina))
        // Sem geometry no texto, ele só descreve a página que a leitura dele daria (A4, 2,54 cm).
        && (r.pagina.is_some() || dividido::mesma_pagina(&featherdocs_core::modelo::ConfigPagina::a4(), &doc.pagina))
        && r.extras.is_none()
        && r.numeracao.is_none_or(|n| n == doc.estilos.numeracao)
        && r.blocos == doc.blocos
}

pub fn nome_da_fonte(fonte: Fonte) -> &'static str {
    match fonte {
        Fonte::Latex => "LaTeX",
        Fonte::Markdown => "Markdown",
    }
}

// ---------------------------------------------------------------- realce de sintaxe

/// Os trechos coloridos de uma linha e se a linha seguinte começa dentro de um bloco ```.
pub fn trechos_da_linha(fonte: Fonte, texto: &str, dentro: bool) -> (Vec<(Range<usize>, Tipo)>, bool) {
    match fonte {
        Fonte::Latex => (realce::latex(texto).into_iter().map(|(a, b, t)| (a..b, t)).collect(), false),
        Fonte::Markdown => {
            let aparada = texto.trim_start();
            let cerca = aparada.starts_with("```") || aparada.starts_with("~~~");
            if dentro || cerca {
                let tudo = if texto.is_empty() { vec![] } else { vec![(0..texto.len(), Tipo::Codigo)] };
                return (tudo, dentro != cerca);
            }
            (realce::markdown(texto).into_iter().map(|(a, b, t)| (a..b, t)).collect(), false)
        }
    }
}

// shortcut: o `text_editor` do pelicano só pinta pelo syntect (extensão + tema); o realce do motor e as
// paletas abaixo ficam para a amostra das Configurações. Ligar quando o pelicano aceitar realce próprio.
/// Os temas do realce (Configurações → Exibição). Cada um: comando, chave, matemática, ambiente,
/// ênfase e link, no claro e no escuro. O `feather_ui::tema` ainda não tem tokens de realce de
/// código; o "Feather" usa a cor de destaque da suíte no comando.
pub const TEMAS: [&str; 4] = ["Feather", "Overleaf", "Monokai", "Solarizado"];
const PALETAS: [([u32; 6], [u32; 6]); 4] = [
    ([0, 0xb05a00, 0x8a2be2, 0x0b7f7f, 0xa0306a, 0x0b7f7f], [0, 0xe5a35a, 0xc39bff, 0x5fd0c8, 0xe88abf, 0x5fd0c8]),
    ([0x1f4fa8, 0x8a5300, 0x0b7a3b, 0x8b1c62, 0x8b1c62, 0x1f4fa8], [0x7aa7ff, 0xe0b36a, 0x7fd49a, 0xe08ac0, 0xe08ac0, 0x7aa7ff]),
    ([0xc4245c, 0x8f7f00, 0x7a4fd6, 0x4f8a10, 0xc56a00, 0x1f8aa3], [0xf92672, 0xe6db74, 0xae81ff, 0xa6e22e, 0xfd971f, 0x66d9ef]),
    ([0x268bd2, 0xb58900, 0x6c71c4, 0x2aa198, 0xd33682, 0xcb4b16], [0x268bd2, 0xb58900, 0x6c71c4, 0x2aa198, 0xd33682, 0xcb4b16]),
];
static TEMA: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn definir_tema(i: usize) {
    TEMA.store(i.min(TEMAS.len() - 1), std::sync::atomic::Ordering::Relaxed);
}

/// As cores do tema `i` no tema claro/escuro atual, na ordem de [`PALETAS`] (para a amostra).
pub fn cores_do_tema(i: usize) -> [Color; 6] {
    let (claro, noturno) = PALETAS[i.min(TEMAS.len() - 1)];
    let p = if feather_ui::tema::escuro() { noturno } else { claro };
    p.map(|v| if v == 0 { feather_ui::tema::COR_AZUL_ACENTO() } else { Color::rgb8((v >> 16) as u8, (v >> 8) as u8, v as u8) })
}

/// O tema do syntect mais perto do tema `i` (o pelicano só tem um tema claro).
pub fn destaque(i: usize) -> pelicano::Highlight {
    use pelicano::Highlight::*;
    let escuro = feather_ui::tema::escuro();
    match i.min(TEMAS.len() - 1) {
        0 if escuro => Base16Ocean,
        1 if escuro => Base16Mocha,
        0 | 1 => InspiredGitHub,
        2 => Base16Eighties,
        _ => SolarizedDark,
    }
}

/// O tema escolhido para o painel aberto.
pub fn tema() -> usize {
    TEMA.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn realce_markdown_atravessa_bloco_cercado() {
        let (_, dentro) = trechos_da_linha(Fonte::Markdown, "```rust", false);
        assert!(dentro);
        let (t, dentro) = trechos_da_linha(Fonte::Markdown, "# não é título", true);
        assert!(dentro);
        assert_eq!(t, vec![(0.."# não é título".len(), Tipo::Codigo)]);
        let (_, dentro) = trechos_da_linha(Fonte::Markdown, "```", true);
        assert!(!dentro);
        let (t, _) = trechos_da_linha(Fonte::Markdown, "# Título", false);
        assert!(t.iter().any(|(_, k)| *k == Tipo::Titulo));
    }

    #[test]
    fn realce_latex_marca_comando_e_comentario() {
        let (t, _) = trechos_da_linha(Fonte::Latex, "\\section{Um} % nota", false);
        assert!(t.iter().any(|(r, k)| *k == Tipo::Comando && *r == (0..8)));
        assert!(t.iter().any(|(_, k)| *k == Tipo::Comentario));
    }

    #[test]
    fn ir_a_linha_posiciona_o_cursor() {
        let texto: String = (0..400).map(|i| format!("linha {i}\n")).collect();
        let mut p = PainelCodigo::novo(Fonte::Markdown, Mapa { texto, blocos: vec![] }, 0);
        // O editor só tem cursor depois de montado (desenhado uma vez).
        let _ = pelicano::capture(&pelicano::Element::<()>::from(pelicano::text_editor(&p.conteudo)),
            pelicano::Size::new(400., 300.), 1., &pelicano::Theme::light(), &mut pelicano::Renderer::new(), None);
        p.ir_a_linha(250);
        assert_eq!(p.linha_do_cursor(), 250);
        p.ir_a_linha(10);
        assert_eq!(p.linha_do_cursor(), 10);
        // Além do fim: para na última linha.
        p.ir_a_linha(5000);
        assert!(p.linha_do_cursor() >= 399);
    }
}
