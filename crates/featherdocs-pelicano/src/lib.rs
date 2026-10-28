//! Interface nativa no pelicano; o documento, a paginação e o histórico permanecem no motor.
pub mod app;
pub mod ajuda;
pub mod boas_vindas;
pub mod inicio;
pub mod arquivos;
pub mod configuracao;
pub mod desenho_compartilhado;
pub mod cor_ui;
pub mod compat;
pub mod dialogos;
pub mod faixa;
pub mod faixa_responsiva;
pub mod menu_classico;
pub mod imagem_compartilhada;
pub mod localidade;
pub mod pagina;
pub mod pdf;
pub mod painel_codigo;
pub mod quadro;
pub mod tinta_docs;

/// As fontes do texto que o programa leva (a interface, a página e o texto dos SVG usam as mesmas).
pub const FONTES_DO_TEXTO: [&[u8]; 10] = [
    include_bytes!("../dados/fontes/LiberationSans-Regular.ttf"),
    include_bytes!("../dados/fontes/LiberationSans-Bold.ttf"),
    include_bytes!("../dados/fontes/LiberationSans-Italic.ttf"),
    include_bytes!("../dados/fontes/LiberationSans-BoldItalic.ttf"),
    include_bytes!("../dados/fontes/LiberationSerif-Regular.ttf"),
    include_bytes!("../dados/fontes/LiberationSerif-Bold.ttf"),
    include_bytes!("../dados/fontes/LiberationSerif-Italic.ttf"),
    include_bytes!("../dados/fontes/LiberationSerif-BoldItalic.ttf"),
    include_bytes!("../dados/fontes/LiberationMono-Regular.ttf"),
    include_bytes!("../dados/fontes/LiberationMono-Bold.ttf"),
];
