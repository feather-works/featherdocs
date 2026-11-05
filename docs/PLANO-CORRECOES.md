# FeatherDocs — plano de correções (auditoria de 18/10/2026)

Achados da auditoria da pasta `featherdocs/` (interface Iced e motores), em
ordem de prioridade. Cada item diz o que corrigir, onde, como conferir e o
custo estimado. Marcar `[X]` ao terminar, como em `PARIDADE-USO.md`.

Carga considerada: uma pessoa, um documento aberto por vez, documentos de
até algumas centenas de páginas.

## Primeiro: perda de dados

- [X] **1. Salvar `.fdoz` pela GUI apaga o histórico de versões.** (18/10: `arquivos::com_historico`, teste `salvar_duas_vezes_guarda_as_duas_versoes`)
  `crates/featherdocs-gui/src/arquivos.rs:40` grava com
  `featherdocs_io::fdoz::gravar`, que não leva o `.git/` embutido. O
  contrato do formato (README, `FORMATO.md` da suíte) diz que as versões
  ficam dentro do arquivo; quem abre um `.fdoz` com histórico e salva pela
  GUI Iced perde tudo.
  - Correção: ler os bytes do arquivo anterior (se existir) e passar por
    `featherdocs_historico::gravar_com_historico(novo, anterior, autor,
    mensagem)`, como a interface antiga fazia em `src-tauri/src/arquivos.rs`.
    A GUI passa a depender de `featherdocs-historico`.
  - Teste: salvar duas vezes o mesmo `.fdoz` e conferir que o segundo tem
    dois commits no `.git/` embutido.
  - Custo: pequeno (uma função e um teste).

## Depois: velocidade em documento grande

- [X] **2. Toda mensagem refaz os quadros de todas as páginas.** (18/10: sem mudança de versão nada é refeito; digitando, só as páginas perto da vista — `refazer_visiveis`; as outras ao rolar até elas)
  `app.rs:373` chama `sessao.quadro(p, u64::MAX)` para cada página a cada
  `atualizar()`, inclusive movimento de mouse e rolagem. Em documento de
  200 páginas isso é trabalho por quadro que cresce com o tamanho.
  - Correção: guardar a versão do documento junto de cada quadro e só
    pedir de novo as páginas cuja versão mudou (o motor já devolve a
    versão; o `u64::MAX` força refazer sempre). Rolagem e mouse não mudam
    a versão.
  - Teste: medir no F12 com o `.docx` de 18 páginas da especificação do
    menu antes e depois; contar quantas páginas foram refeitas por tecla.
  - Custo: médio.

- [-] **3. Os canvas das páginas não têm `canvas::Cache`.** (05/11: não se aplica: a interface Iced saiu; o pelicano só repinta o que mudou, ver `compat.rs`)
  Cada redesenho do Iced refaz a geometria de todas as operações da página.
  - Correção: um `canvas::Cache` por página, limpo só quando o quadro da
    página muda (depende do item 2).
  - Custo: pequeno depois do item 2.

- [X] **4. O painel de revisão recalcula tudo a cada `view`.** (05/11: a contagem fica guardada por versão do documento e cursor, campo `App::contagem`)
  `app.rs:1997` (`view_revisao`) conta palavras e percorre comentários e
  revisões em toda chamada de `view`.
  - Correção: calcular em `atualizar()` só quando a versão do documento
    muda e guardar o resultado no `App`.
  - Custo: pequeno.

## Depois: interface

- [X] **5. O menu do botão direito pode sair da janela.** (05/11: na interface pelicano, `view_contexto` limita o menu à janela)
  `app.rs:1951` (`view_contexto`) posiciona o menu no ponto do clique sem
  limitar à área visível; clicando perto da borda direita ou de baixo, parte
  do menu fica fora da tela.
  - Correção: limitar `x` e `y` ao tamanho da janela menos o tamanho do
    menu (a janela já chega em `window::Event::Resized`).
  - Custo: pequeno.

- [X] **6. Recentes guardam arquivos que já não existem.** (05/11: no pelicano o recente sumido aparece cinza, "Arquivo não encontrado", não abre e tem o X para tirar)
  `configuracao.rs:69` (`lembrar`) nunca remove caminhos apagados ou
  movidos; o menu mostra a entrada e abrir dá erro.
  - Correção: filtrar com `Path::exists` ao montar o menu e tirar da lista
    ao falhar a abertura.
  - Custo: pequeno.

## Limpeza (sem efeito para quem usa)

- [X] **7. Código morto no leitor LaTeX.** (18/10: `MARCA_DE_ROTULO` e `rotular` estão em uso; saíram `numeros`, `contadores` e `contar`)
  `crates/featherdocs-io/src/latex/avisos.rs`: `MARCA_DE_ROTULO`, `rotular`,
  `contar` e os campos `rotulos`, `numeros`, `contadores` não são usados
  (o compilador avisa). Apagar, depois de conferir com `grep` que nada os
  chama.

- [X] **8. Import sem uso no motor.**
  `crates/featherdocs-core/src/sessao.rs:17` importa `compor_em` sem usar.
  Apagar.

## Segunda auditoria (18/10/2026, tarde) — feito

- [X] Bomba de ZIP: `.docx` e `.fdoz` descompactam cada parte só até 256 MiB (`featherdocs_io::ler_limitado`).
- [X] Autor-entidade do `.bib` (`{{Agência Nacional…}}`) e "and" dentro de chaves; a gravação devolve as chaves.
- [X] A marca "quebra de página/coluna/seção" e a grade não saem no PDF (`Sessao::quadro_impresso`).
- [X] `\%`, `\&`, `\_` e `\#` dentro de `\text{}` nas fórmulas.
- [X] Tabela do LaTeX que cabe numa página não se parte.
- [X] `\appendix`: títulos em letras (A, A.1), `\ref` acompanha, o `.fdoz` guarda (`Atrib.apendice`) e o `.tex` exportado escreve.
- [X] Abrir link/PDF no sistema não deixa processo "defunto".
- [X] Interface Tauri/TypeScript antiga e `featherdocs-ponte` apagadas; os textos da ajuda e as fontes foram para `crates/featherdocs-gui/dados/`.

## Já resolvido na separação do repositório (18/10/2026)

- [X] Dependências por pasta do monorepo trocadas por tags git
  (`feather-io` v0.1.1, `feather-charts` v0.3.1, `feather-ui` v0.2.9).
  `feather-math` e a parte "docs" do `featheroffice-table-adapters` só
  eram usadas aqui e vieram para `crates/feather-math` e
  `crates/featherdocs-tabelas`.
- [X] Teste `fonte_renumerada_no_docx_e_religada_sem_perder_o_svg` pedia
  pizza com duas séries (o motor de gráficos só aceita uma); passou a
  usar barras, que é o que o teste quer conferir.
- [X] Teste de nota dentro de tabela comparava o rótulo antes da
  numeração automática das notas.
- [X] `feather-draw` v0.1.2 (Times, Courier, imagem e texto girado no
  PDF) publicado; a GUI usa a tag, sem `[patch]`. O `featherdocs-io` fica
  na v0.1.1 porque passa cores ao `feather-charts`, que ainda usa a v0.1.1;
  subir os dois juntos quando sair um `feather-charts` com a v0.1.2.
- [ ] No monorepo, tirar a parte "docs" de `featheroffice-table-adapters`
  (agora duplicada aqui) e a pasta `feather-math`.

## Auditoria de 05/11/2026

- [X] **9. Salvar apagava o histórico sem aviso.** Com o `.fdoz` anterior preso por outro programa, a leitura falhava e a gravação recomeçava as versões. Agora `arquivo::ler_se_existir` faz desse caso um erro (nada é gravado) e o histórico danificado recomeça com aviso na barra (`gravar_versao` devolve `true`). Vale para o app e para o `featherdocs-mcp`.
- [X] **10. A correção ao digitar mexia em código e fórmulas.** Pula trechos com `CODIGO`/`MATEMATICA` e o código entre crases ainda aberto.
- [X] **11. Estrutura, comentários e alterações eram refeitos a cada quadro.** Guardados pela versão (`App::pela_versao`); cada sessão tem a própria faixa de versões, então abrir outro arquivo também refaz.
- [X] **12. A cópia de recuperação não ia até o disco antes do `rename`.** Usa `featherdocs_io::arquivo::gravar`.
- [X] **13. No Windows a segunda impressão podia falhar** (o PDF temporário tinha nome fixo). Um nome por impressão.
- [X] **14. A ponte MCP lia linha sem teto antes da senha.** Teto de 64 MiB por pedido.
- [X] **15. Alt+Shift+↓ levava um parágrafo a mais** quando a seleção terminava no começo do seguinte.
- [X] **16. Código morto do Iced em `compat.rs`** (`CacheCanvas`, `ListaPropria` com `unsafe`, `estilo_menu_da_faixa`): apagado.
