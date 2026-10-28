use featherdocs_pelicano::app::App;

fn main() -> Result<(), pelicano::Error> {
    // A página e a interface usam as fontes que o programa leva (o `.font()` do Iced).
    for f in featherdocs_pelicano::FONTES_DO_TEXTO {
        pelicano::load_font(f.to_vec());
    }
    // O pelicano roda as tarefas no tokio dele; este dá um contexto ao que o `update` chama direto.
    let rt = tokio::runtime::Runtime::new().expect("falha ao iniciar o tokio");
    let _guarda = rt.enter();
    // Várias janelas; a principal abre pelo `App::novo`, com a moldura que a configuração pede.
    pelicano::run(pelicano::Settings { start_hidden: true, ..Default::default() }, App::novo)
}
