# Plano: `.fdoz` ↔ `.docx` completo

28/10/2026. Base: `docs/PARIDADE-DOCX.md` (209 recursos medidos; só 82 completos nas três
colunas). Decisão do usuário: o principal do FeatherDocs é a ida e volta `.fdoz` ↔ `.docx`;
LaTeX e Markdown são o extra.

## A ideia: ler e gravar são a mesma tarefa

Cada recurso do Word é **uma** tarefa, não duas: o campo no modelo e na coluna do `.fdoz`, a
leitura e a gravação saem juntos, e a tarefa só termina quando a linha dele no
`paridade_docx` fica `[x] [x] [x]`. Nada de ler um recurso que não se grava (é o caso da
mescla horizontal hoje: gravada e não lida) nem o contrário.

Para isso não se repetir à mão, as propriedades que são uma lista de atributos (as do
caractere `w:rPr`, do parágrafo `w:pPr`, da tabela `w:tblPr`, da célula `w:tcPr` e da seção
`w:sectPr`) passam a sair de **um mapa só**: cada entrada diz o campo do modelo, o elemento e o
atributo do OOXML e a conversão (twips, meios-pontos, cor, liga/desliga). O leitor e o gravador
percorrem o mesmo mapa; acrescentar uma propriedade é uma linha no mapa, e ela já vai e volta.
O que é estrutura (tabela, imagem, seção, nota) continua escrito à parte, mas na mesma tarefa.

## Por que o modelo muda primeiro

O modelo nasceu do LaTeX, e muitas lacunas não são do leitor nem do gravador: o modelo não tem
onde guardar.

| Hoje | O Word tem | Muda para |
|---|---|---|
| `Celula = Vec<Trecho>` (só texto) | parágrafos, listas, imagens e tabelas dentro da célula, mescla h/v, largura, bordas, fundo | célula com blocos e propriedades |
| Cabeçalho/rodapé = 3 textos (`Faixas`, estilo `fancyhdr`) | blocos de verdade (tabela, imagem, letra), primeira página, pares/ímpares, um por seção | cabeçalho e rodapé em blocos, por seção |
| Uma `ConfigPagina` no documento | várias seções, cada uma com papel, margens, colunas, numeração | lista de seções |
| `Estilo` = bits sobre "Título N" | estilos de parágrafo, caractere, tabela e lista, com herança (`basedOn`) e o padrão do documento | estilos como no Word |
| `Fmt` com tamanho, cor, realce, posição; família só como "serifa/sem serifa/código" | nome da fonte, tipos de sublinhado, versalete, espaçamento… | `Fmt` com as propriedades do `w:rPr` |

**Compatibilidade:** todo campo novo entra no `.fdoz` como coluna Arrow **opcional** (como o
`dono` dos trechos já faz): `.fdoz` antigo continua abrindo, e o que faltar vem com o valor de
hoje. O que muda de forma (célula, cabeçalho) é lido do jeito antigo e convertido ao abrir.
LaTeX e Markdown continuam passando nos testes deles a cada fase.

## Rede de segurança, antes de tudo

1. **Catraca da paridade**: um teste roda o `paridade_docx` e falha se algum item piorar
   (um `[x]` que vira `[~]`). Cada fase só pode subir os números.
2. **Sacola de interoperabilidade**: o que o modelo ainda não entende (forma, gráfico,
   SmartArt, objeto OLE, controle de conteúdo, tema) é guardado como o XML original, preso ao
   lugar onde estava, e devolvido igual ao gravar. Aparece na tela como um quadro "objeto do
   Word" e não some. É o que o LibreOffice faz (`InteropGrabBag`). Com isso a coluna "Volta
   igual" passa já no começo para esses itens, enquanto o suporte de verdade não chega.
3. **Arquivos reais**: os `.docx` do usuário entram num corpus (fora do git se forem
   privados) e rodam o mesmo critério da paridade em cada fase.

## Fases (em ordem: o que todo documento de trabalho usa vem primeiro)

| Fase | O quê | Lacunas do top 10 | Itens da paridade |
|---|---|---|---|
| 0 ✅ | Mapa único rPr/pPr, catraca, sacola de interoperabilidade (entregue em 28/10/2026, ver abaixo) | — | §7 volta igual pela sacola; §16 fica para as fases 5 e 8 |
| 1 ✅ | **Estilos**: estilos de parágrafo e caractere com nome, herança, `docDefaults`, Normal modificado, aparência dos títulos do `styles.xml`, Title/Subtitle; **nome da fonte** (e fonte do tema) (entregue em 28/10/2026, ver abaixo) | 1, 2, 8 | §3 menos o estilo de tabela (fase 4), fonte em §1 |
| 2 ✅ | **Caractere e parágrafo completos** pelo mapa (entregue em 28/10/2026, ver abaixo): tipos e cor do sublinhado, tachado duplo, versalete, maiúsculas, espaçamento, escala, oculto; entrelinha exata/mínima, espaço automático, manter com o próximo, linhas juntas, órfãs, tabulações, nível de tópico, capitular | — | §1 e §2 |
| 3 ✅ | **Listas**: número inicial, reinício, continuar depois de parágrafo, texto do número ("Passo %1:"), marcador de símbolo, estilo de lista, numeração vinda do estilo (List Number) (entregue em 28/10/2026, ver abaixo) | 7 | §4 |
| 4 ✅ | **Tabelas** (entregue em 28 e 29/10/2026, ver abaixo): célula com blocos, mescla horizontal lida, mescla vertical, larguras das colunas e da tabela, bordas da tabela e por célula, fundo, alinhamento vertical, altura da linha, margens, estilo de tabela, tabela aninhada | 3, 4, 9 | §5 |
| 5 ✅ | **Seções, cabeçalho e rodapé** (entregue em 29/10/2026, ver abaixo): lista de seções (papel, margens, colunas, medianiz, distância do cabeçalho, número e formato da página), cabeçalho/rodapé em blocos, primeira página, pares/ímpares, um por seção, marca d'água | 6, 10 | §8, §9, marca d'água de §16 |
| 6 ✅ | **Imagens e objetos** (entregue em 29/10/2026, ver abaixo): imagem no meio do texto, flutuante (âncora, posição, texto contornando), texto alternativo, recorte, rotação, VML antiga; caixa de texto de verdade; controles de conteúdo (texto, caixa, lista, data) | 5 | §6, §7 |
| 7 🟡 | **Notas, comentários, revisão, campos e referências** (primeira parte entregue em 29/10/2026, ver abaixo): vários parágrafos em nota e comentário, formato das notas, indicadores, REF/PAGEREF, DATE com formato, HYPERLINK de campo, MERGEFIELD, entradas guardadas do sumário, legenda com rótulo próprio; alterações de formatação, de parágrafo, movidas e de linha de tabela | — | §10 a §15 |
| 8 🟡 | **Documento** (entregue em 29/10/2026 menos as fontes embutidas, ver abaixo): propriedades (core, app, custom), idioma por trecho e padrão, direita-para-esquerda, hifenização, tabulação padrão, fontes embutidas | — | §16 |

Gráfico, SmartArt, formas e objetos OLE ficam na sacola (fase 0) até haver motivo para
editá-los; gráfico pode vir depois pelo `feather-charts`, que o FeatherSheet já usa.

## Fase 0 — entregue (28/10/2026, ramo `fase0-docx`)

| Coluna | Antes `[x]` / `[~]` / `[ ]` | Depois |
|---|---|---|
| Lê | 84 / 44 / 81 | 84 / 48 / 77 |
| Grava | 84 / 39 / 86 | **92** / 38 / 79 |
| Volta igual | 128 / 0 / 81 | **132** / 0 / 77 |

Completos nas três colunas: 82 → 82 (a sacola guarda, não entende: o Lê dos itens dela fica `[~]`
"guardado na sacola").

- **Catraca**: `crates/featherdocs-io/tests/paridade_docx.rs` usa o próprio exemplo `paridade_docx`
  como módulo (`#[path]`, sem duplicar a medição) e falha se algum item piorar em relação a
  `crates/featherdocs-io/dados/paridade_docx_base.txt` (uma linha por item, as três marcas).
  Melhorou: `PARIDADE_ATUALIZAR=1 cargo test --test paridade_docx` regrava a base.
- **Mapa único**: `crates/featherdocs-io/src/docx/mapa.rs`, tabelas `CARACTERE` (`w:b`, `w:i`,
  `w:strike`, `w:color`, `w:sz`/`w:szCs`, `w:highlight` só lido, `w:u`, `w:shd`, `w:vertAlign`) e
  `PARAGRAFO` (`w:shd`, `w:spacing` antes/depois/entrelinha, `w:ind` esquerda/direita/primeira
  linha, `w:jc`); o leitor (`prop_caractere`, `prop_paragrafo`, estilos) e o gravador (`rpr`, `ppr`,
  estilos com nome) percorrem as mesmas tabelas. Conferido: os 209 `.docx` lidos e gravados saem
  byte a byte iguais aos de antes. **Fora do mapa**: `w:rFonts` (o nome da fonte vira os bits
  `SERIFA`/`CODIGO` por semelhança, não é conversão de valor; entra com o nome da fonte na fase 1),
  `w:rStyle`, `w:pStyle`, `w:numPr` (apontam estilos e numerações) e `w:pBdr` (quatro filhos numa
  máscara de bits com uma cor só; entra quando as bordas tiverem lado próprio). `w:tblPr`, `w:tcPr`
  e `w:sectPr` entram no mapa nas fases 4 e 5, quando o modelo tiver os campos.
- **Sacola**: `crates/featherdocs-io/src/docx/sacola.rs`. `Marca::ObjetoWord { xml, partes, resumo }`
  guarda o `w:drawing` (forma sem caixa de texto, gráfico, SmartArt, grupo, tela de desenho) e o
  `w:object` (OLE) com as declarações `xmlns` da raiz que o XML usa e as partes do pacote que ele
  aponta, seguindo as relações delas (`chartN.xml`, `_rels`, o `.xlsx`, o `.bin`, a prévia); o
  gravador renumera os `r:id` e põe as partes e os `ContentType` no pacote. `Marca::Controle
  { xml, fim, bloco }` guarda o `w:sdtPr` em volta do texto do controle, que continua texto
  (controle de linha e de bloco). No `.fdoz`: marcas 7 e 8 nas colunas de sempre e
  `data/sacola.arrow` (opcional) com as partes. A tela mostra "[gráfico do Word]"; LaTeX e Markdown
  põem um comentário no lugar. O `.fdoz` de antes da fase (`dados/antes_fase0.fdoz`) abre igual
  (`tests/fdoz_antigo.rs`).
- **Ainda não vai para a sacola**: caixa de texto e imagem (o leitor de sempre cuida), VML solta
  (`w:pict`), o tema (`theme1.xml`), objetos em notas, comentários e cabeçalhos (o leitor deles não
  tem as relações à mão), controle com `w:docPartObj` (sumário, bibliografia, capa: o FeatherDocs
  gera) e controle de bloco dentro de tabela (fase 4). O desenho do SmartArt (`diagramDrawing`,
  apontado de dentro da parte de dados) não é levado: o Word o refaz.
- **LibreOffice**: o `.docx` gravado com forma, OLE e controles abre no `libreoffice --headless`; o
  gráfico abre quando o `chartN.xml` é um gráfico completo (o do exemplo é mínimo demais e não abre
  nem no original).

## Fase 1 — entregue (28/10/2026, ramo `fase1-docx`)

| Coluna | Antes `[x]` / `[~]` / `[ ]` | Depois |
|---|---|---|
| Lê | 84 / 48 / 77 | **96** / 44 / 69 |
| Grava | 92 / 38 / 79 | **104** / 35 / 70 |
| Volta igual | 132 / 0 / 77 | **140** / 0 / 69 |

Completos nas três colunas: 82 → **94**. §3: 11 de 12 em `[x] [x] [x]`; o estilo de tabela (`w:tblStyle`)
fica para a fase 4 (o modelo não tem tabela com estilo). §1: os quatro itens de fonte (família, Calibri,
Consolas, fonte de tema) em `[x] [x] [x]`.

- **Modelo** (`featherdocs-core/src/modelo.rs`): `Estilo` ganhou `codigo` (o `styleId` do Word; vazio =
  `FDEstilo<id>`), `caractere`, `herda` (`w:basedOn`), `proximo` (`w:next`), `familia` e o resto do mapa
  (`realce`, `posicao`, `fundo`, recuos); guarda só o que o estilo define. `Estilos` ganhou `padrao`
  (`w:docDefaults`) e `tema` (o `theme1.xml` como veio). `Estilos::resolvido` soma a herança sobre o
  padrão; `resolvido_do_bloco` é o que o motor desenha (o do bloco, ou o Normal/padrão no parágrafo sem
  estilo). `Fmt` ganhou `familia` (o nome da fonte, ou `+majorHAnsi`/`+minorHAnsi` para a do tema) e
  `estilo_de_caractere`. `Familia` é um índice num catálogo de nomes para o `Fmt` continuar `Copy`.
- **`.fdoz`**: colunas opcionais `familia` e `estilo_car` em `trechos.arrow`; em `estilos.arrow`, `codigo`,
  `caractere`, `herda`, `proximo`, `familia`, `realce`, `posicao`, `fundo`, `recuo_esq`, `recuo_dir`,
  `primeira`, a linha de id 0 para o padrão e o tema nos metadados (`tema`). O `.fdoz` de antes da fase 0
  (sem nenhuma delas) abre igual (`tests/fdoz_antigo.rs`).
- **Leitor e gravador**: o `w:rFonts` entrou no mapa (`Conv::Familia`: `w:ascii`/`w:hAnsi`, e o
  `w:asciiTheme`/`w:hAnsiTheme`, que ganha). Entram do `styles.xml` os estilos de parágrafo e de caractere,
  menos os iguais aos que o gravador põe por conta própria (o documento do FeatherDocs volta sem estilos a
  mais) e o "Default Paragraph Font"; o `styleId` dos que o gravador conhece é traduzido ("Ttulo1" →
  "Heading1") e o do Word toma o lugar do embutido ao gravar. O parágrafo começa com o que o estilo do Word
  (com a herança e o padrão) define e o `w:pPr` direto vem por cima; o gravador põe explícito o recuo zero
  e o alinhamento à esquerda que desfazem o do estilo. O tema volta como parte do pacote.
- **Motor**: desenha com o estilo resolvido (`Entorno.estilo`). A fonte pelo nome só escolhe a classe
  (serifa, sem serifa, código) com que o motor mede e a tela desenha: as medidas são as tabelas das
  Liberation, e o motor não lê fontes do sistema (`classe_da_familia`).
- **Limites** (`// shortcut:` no código): a herança só liga bits (o `<w:b w:val="0"/>` do filho não desliga
  o negrito do pai; o alinhamento à esquerda do filho não desfaz o do pai); a letra do estilo de caractere
  fica copiada no trecho; o catálogo de nomes de fonte não encolhe. O título com `w:outlineLvl` direto e
  sem estilo volta com o estilo `Heading N` do Word (converge na segunda volta). Do tema só as fontes são
  entendidas; uma imagem presa ao tema não vai junto.
- **Arquivos reais**: 23 `.docx` da máquina do usuário (fora do git). Os estilos voltam iguais pelo `.docx`
  e pelo `.fdoz` nos 23; os blocos voltam iguais pelo `.fdoz` nos 23 e pelo `.docx` em 19 (os outros 4
  diferem em tabelas, fase 4, e no título por `w:outlineLvl` acima). Cinco dos gravados abrem no
  `libreoffice --headless` e o PDF usa as mesmas fontes do original.

## Fase 2 — entregue (28/10/2026, ramo `fase2-docx`)

| Coluna | Antes `[x]` / `[~]` / `[ ]` | Depois |
|---|---|---|
| Lê | 96 / 44 / 69 | **121** / 36 / 52 |
| Grava | 104 / 35 / 70 | **131** / 25 / 53 |
| Volta igual | 140 / 0 / 69 | **157** / 0 / 52 |

Completos nas três colunas: 94 → **121**. §1 (27 itens) e §2 (24 itens) todos em `[x] [x] [x]`.

- **Modelo** (`featherdocs-core/src/modelo.rs`): `Fmt.mais: MaisLetra` (efeitos em bits — maiúsculas,
  versalete, tachado duplo, contorno, sombra, relevo, baixo-relevo, oculto —, tipo e cor do sublinhado,
  elevação `w:position`, espaçamento entre letras, escala, kerning, a cor do tema e o nome do marca-texto
  de que a cor saiu); `Atrib.mais: MaisParagrafo` (bits de fluxo — manter com o próximo, linhas juntas,
  quebra antes, sem controle de viúvas, espaço contextual, espaço automático antes/depois, distribuído —,
  entrelinha mínima e exata em pontos, traço e espessura da borda, nível de tópico, capitular, até 16
  paradas de tabulação). Os dois são `Copy`, como o `Fmt` e o `Atrib`. `Estilo` ganhou `mais_letra`,
  `mais_paragrafo` e `desligados`.
- **Herança com "desligar"** (o limite da fase 1): `Estilo::desligados` guarda o que o estilo define como
  desligado (os bits do trecho, os efeitos, o fluxo e `ALINHAMENTO_DEFINIDO`, o `w:jc` à esquerda). O
  leitor descobre isso lendo cada propriedade do estilo também sobre "tudo ligado"; `Estilos::resolvido`
  aplica, e o gravador devolve `<w:b w:val="0"/>`, `<w:keepNext w:val="0"/>`, `<w:jc w:val="left"/>`.
  No parágrafo, o que o estilo liga e o parágrafo não sai desligado à vista (senão o estilo voltaria ao ler).
  Testes: `estilos::testes::o_filho_desliga_o_que_o_pai_liga` (core) e
  `docx::testes::estilo_filho_desliga_o_do_pai_e_volta` (`.docx` e `.fdoz`).
- **`.fdoz`**: colunas opcionais `mais` em `trechos.arrow` e em `blocos.arrow`, `mais_letra`,
  `mais_paragrafo` e `desligados` em `estilos.arrow` (as duas `mais*` em JSON, nulo = padrão).
  `tests/fdoz_antigo.rs` passa (o JSON do modelo não muda para quem não usa os campos novos).
- **Mapa**: todas as propriedades novas entram por `docx/mapa.rs` (convs novas `Desligado`, `Capitular`,
  `Entrelinha` com as três regras, `CorComNome` que grava, `Fora`). `w:numPr`, `w:pBdr` e `w:tabs` têm
  filhos e são lidos e gravados à parte, mas ficam no mapa como `Conv::Fora` para sair na ordem do esquema.
  O `w:pBdr` não entrou no mapa porque o modelo tem um traço e uma espessura para os quatro lados (o Word
  tem um por lado; `// shortcut:` em `MaisParagrafo`).
- **Mudanças de leitura**: o `w:pageBreakBefore` vira o bit do parágrafo (antes era um bloco
  `QuebraDePagina` a mais; o `w:br` de página continua bloco): a tela mostra a marca "quebra de página"
  no fim da página de antes (só na tela, como a do bloco), e o LaTeX e o Markdown exportados põem a quebra
  antes do parágrafo. O `w:outlineLvl` direto num parágrafo sem estilo de título fica parágrafo com
  `nivel_topico` (antes virava Título e mudava a letra) e, como no Word, entra no sumário, na estrutura e
  nos marcadores do PDF pelo nível (`layout::texto_do_titulo`).
- **Motor** (`layout.rs`, `paginacao.rs`): desenha maiúsculas e versalete (letra a letra; o versalete
  encolhe a palavra toda — `// shortcut:`), tachado duplo, sublinhado duplo e ondulado (`Deco::Traco`),
  elevação, entrelinha exata e "pelo menos", justificado distribuído (a última linha estica), e na
  paginação manter com o próximo, linhas juntas, sem viúvas e quebra antes. Ficam só na ida e volta:
  espaçamento, escala, kerning, oculto (o texto aparece), contorno/sombra/relevo, cor do tema, espaço
  automático e contextual, tipo/espessura da borda, tabulações personalizadas (a tabulação continua
  encostando o resto à direita), capitular (fica num parágrafo próprio, como no arquivo).
- **Arquivos reais** (os mesmos 23 `.docx`): estilos iguais pelo `.docx` e pelo `.fdoz` nos 23; blocos
  iguais pelo `.fdoz` nos 23 e pelo `.docx` em 19 (os mesmos 4 de antes, em tabelas: fase 4). Na maioria deles
  algum parágrafo agora traz o que antes se perdia (manter com o próximo, entrelinha exata, tabulações…).
  Um título feito com `w:outlineLvl` direto (num dos 23) passa a parágrafo com nível de tópico (continua no
  sumário e na estrutura). Onze dos
  gravados abrem no `libreoffice --headless`.

## Fase 3 — entregue (28/10/2026, ramo `fase3-docx`)

| Coluna | Antes `[x]` / `[~]` / `[ ]` | Depois |
|---|---|---|
| Lê | 121 / 36 / 52 | **128** / 33 / 48 |
| Grava | 131 / 25 / 53 | **138** / 22 / 49 |
| Volta igual | 157 / 0 / 52 | **161** / 0 / 48 |

Completos nas três colunas: 121 → **128**. §4 (15 itens) toda em `[x] [x] [x]`. A lista dentro de célula (§5)
continua `[~]`: a célula ainda é só texto (fase 4).

- **Modelo** (`featherdocs-core/src/modelo.rs`): `Estilos::listas: Vec<ListaDoWord>` (um `w:abstractNum`: os níveis
  `NivelDeLista` — `formato` (`w:numFmt`), `texto` (`w:lvlText`), `inicio` (`w:start`), `recuo`, `pendurado`,
  `fonte` do marcador — e `estilo`, o `w:numStyleLink`). `MaisParagrafo` ganhou `lista` (índice + 1 em
  `Estilos::listas`; 0 = a lista do FeatherDocs de sempre), `nivel_lista` (o `w:ilvl` de um estilo de parágrafo
  que numera) e `inicio` (o número deste item). O `Atrib::reiniciar` de antes vale nas duas. `Atrib::limpar_lista`
  tira tudo isso quando a tela troca o tipo do bloco ou o estilo da lista (aí o item volta à lista do FeatherDocs).
- **Duas contagens**: a lista do FeatherDocs continua "a sequência de itens vizinhos" (recomeça depois de um
  parágrafo que não é recuado). A do Word conta por lista pelo documento todo, atravessando parágrafos, com cada
  nível recomeçando depois de um nível de cima, e o `%1.%2` montado com o formato de cada nível
  (`paginacao::marcadores`, que agora recebe `Estilos::listas`).
- **Leitor**: lê do `numbering.xml` os níveis inteiros, os `w:num` com `w:startOverride` e o `w:numStyleLink`
  (seguido pelo estilo de lista do `styles.xml` até a definição). Cada `abstractNum` usado vira uma `ListaDoWord`,
  menos os iguais aos que o gravador do FeatherDocs põe (`docx_gravar::nivel_nativo`): o documento do FeatherDocs
  volta com as listas dele, sem nenhuma do Word. O `startOverride` vale no primeiro item do `numId`: vira
  `reiniciar` (ou `inicio`, se recomeça noutro número; na lista do FeatherDocs, o item logo depois de um parágrafo
  não ganha `reiniciar`, que já é implícito). O `w:numPr` de um estilo de parágrafo (List Number) entra no
  `MaisParagrafo` do estilo e o parágrafo com ele vira item; o `numId` 0 direto tira a lista do estilo.
- **Gravador**: cada `ListaDoWord` vira o `abstractNum` 200+i e o `num` 20000+i; o item que recomeça (`reiniciar`,
  `inicio`) ganha um `w:num` novo com `w:startOverride`; a lista com `estilo` sai como no Word (o `abstractNum`
  com `w:numStyleLink`, a definição com `w:styleLink`, e o estilo de lista no `styles.xml`). O estilo de parágrafo
  que numera leva o `w:numPr`; o parágrafo com esse estilo que não é item sai com `numId` 0. Na lista do
  FeatherDocs, o `inicio` também sai como `startOverride`, e o parágrafo recuado no meio dela não a interrompe
  mais (antes o Word recomeçava a numeração ali e a tela não).
- **`.fdoz`**: `data/listas.arrow` opcional (um nível por linha: `lista`, `estilo`, `formato`, `texto`, `fonte`,
  `inicio`, `recuo`, `pendurado`); `lista`, `nivel_lista` e `inicio` vão na coluna JSON `mais` dos blocos e dos
  estilos que já existia. `tests/fdoz_antigo.rs` passa.
- **Motor** (`paginacao.rs`, `sessao.rs`): número inicial, reinício, continuação depois de parágrafo, texto do
  número e o marcador de símbolo (o caractere da Wingdings/Symbol no trecho U+F0xx, ou o "o" da Courier New,
  trocado pelo que as fontes do motor têm: ▪ ► ◆ ● □ ◦ •). O rótulo de mais de 4 caracteres ("Passo 1:") se
  compõe como o rótulo de texto do LaTeX (`item_com_rotulo`: empurra a primeira linha). A tarefa (☐) continua
  com a caixa do FeatherDocs. Teste: `paginacao::testes::lista_do_word_comeca_reinicia_continua_e_usa_o_texto_do_nivel`.
- **LaTeX e Markdown**: o Markdown lê o `5.` como `inicio` e o grava de volta (só no primeiro item: o Markdown não
  tem como dizer o número de um item do meio); o LaTeX lê o `start=` do `enumitem` e exporta o `inicio` como
  `\setcounter{enumi}{4}` antes do `\item`. A medição do LaTeX (`paridade_latex`) sai igual à do `main`.
- **Limites** (`// shortcut:` no código): do `w:lvl` não entram o `rPr` do número além da fonte, `w:suff`,
  `w:lvlJc`, `w:lvlRestart` e o `w:lvlOverride` com nível inteiro; o recuo do nível vai e volta mas o motor recua
  os itens como sempre (18 pt por nível); a contagem é por `abstractNum`: o item de um `w:num` antigo que volta depois
  de um reinício segue do número do reinício (no Word, cada `w:num` com `startOverride` conta à parte); o estilo de lista volta com o `styleId` no lugar do nome; o LaTeX e o
  Markdown não levam o texto do número nem a continuação da lista do Word depois de um parágrafo; aplicar na tela
  um estilo que numera não transforma o parágrafo em item.
- **Arquivos reais** (os mesmos 23 `.docx`): os rótulos das listas voltam iguais pelo `.fdoz` e pelo `.docx` nos 23;
  os blocos voltam iguais pelo `.fdoz` nos 23 e pelo `.docx` em 19 (os mesmos 4 de antes, em tabelas). Em 5 deles
  os rótulos mudaram em relação a antes e agora batem com o que o LibreOffice mostra: a numeração que continua
  depois dos marcadores ("5." em vez de "1."), o "II." de volta ao nível de cima com o subnível recomeçando em
  "i.", e os marcadores de símbolo (● ► ▪ no lugar do • de sempre). Oito dos gravados abrem no
  `libreoffice --headless`, com a mesma numeração do original.

## Fase 4, primeira parte — entregue (28/10/2026, ramo `paridade-tabelas`)

- **Modelo:** `PropsTabela` no `Bloco` (campo `tabela`; vazio fora de tabela): larguras das colunas
  em pontos, largura em porcentagem, layout fixo, os seis fios da tabela (`BordasDeTabela`/`FioDeBorda`),
  alturas das linhas (mínima ou exata) e, por célula (`PropsCelula`, na coordenada da grade), fundo,
  alinhamento vertical e do texto, os quatro fios, margens, direção do texto e mescla vertical
  (`linhas`: a célula ocupa as de baixo, que ficam vazias). O alinhamento da tabela na página é o
  `atrib.alinhamento` do bloco. No `.fdoz`, coluna opcional `tabela` em JSON (os arquivos antigos abrem).
- **Leitor:** `gridSpan` vira `MesclagemTabela` com as células encobertas vazias no modelo (antes a
  linha ficava com menos células e as colunas desalinhavam); `vMerge`, `tblGrid`, `tblW pct`, `jc`,
  `tblLayout`, `tblBorders`, `tcBorders`, `shd`, `vAlign`, `tcMar`, `textDirection`, `trHeight` e o `jc`
  do primeiro parágrafo da célula. O cinza do cabeçalho e a grade cinza que o próprio FeatherDocs grava
  voltam como "sem fundo"/"sem bordas", para a ida e volta não inventar formatação.
- **Gravador:** tudo isso na ordem do esquema do Word; bordas próprias saem no lugar da grade padrão.
- **Tela:** a composição (`layout::compor_em_tabela`) usa as larguras (encolhidas se não cabem),
  centraliza ou alinha à direita, pinta o fundo por célula, respeita altura e alinhamento vertical,
  desenha os fios da tabela e das células e não traça a divisa dentro da mescla vertical. Fio duplo,
  tracejado e pontilhado saem como traço cheio (`shortcut:` no código).
- **Edição:** inserir/excluir linha e coluna desloca as propriedades junto (`PropsTabela::inserir_linha`…,
  também pelo `featherdocs-tabelas`); com mescla vertical, como com a horizontal, pede para desmesclar antes.
- **Medido:** §5 de 3 para 17 de 21 completos; total de 128 para **142 / 209**. Faltam vários
  parágrafos e lista na célula, imagem na célula e tabela aninhada (todos pedem célula com blocos) e
  o estilo de tabela (§3).

## Fase 4, segunda parte — entregue (29/10/2026)

| Coluna | Antes `[x]` / `[~]` / `[ ]` | Depois |
|---|---|---|
| Lê | 142 / 34 / 33 | **147** / 31 / 31 |
| Grava | 151 / 23 / 35 | **156** / 20 / 33 |
| Volta igual | 176 / 0 / 33 | **178** / 0 / 31 |

Completos nas três colunas: 142 → **147**. §3 (12) e §5 (21) inteiras em `[x] [x] [x]`.

- **Célula com blocos** (`featherdocs-core/src/modelo.rs`): `PropsCelula::blocos` guarda o conteúdo da célula
  quando não é um parágrafo só de texto (vários parágrafos, lista, imagem, tabela dentro, que pode ter célula com
  blocos também). A `Celula` continua sendo texto: o dos blocos (`texto_dos_blocos`: um parágrafo por linha com
  `QUEBRA_DE_LINHA`, a imagem no meio da linha, a tabela de dentro com as células separadas por espaço), que a tela
  desenha e edita. `PropsCelula::blocos_valem` compara letra a letra (formatação e objetos juntos): editado o texto
  da célula, o gravador grava o texto e os blocos velhos não entram. `cada_bloco_mut` percorre os blocos de dentro
  das células (imagens, notas e sacola do leitor passam por eles).
- **Leitor**: as tabelas abertas são uma pilha (`Leitor::tabelas`); ao abrir a célula, os blocos de fora são
  guardados e os parágrafos da célula passam pelo mesmo `emitir` do corpo (listas, imagens, títulos) até o
  `w:tc` fechar. A tabela de dentro é lida inteira, com as propriedades dela.
- **Gravador**: a célula com blocos válidos sai bloco a bloco (`blocos_da_celula`, sem mexer na contagem da lista
  de fora) e termina num parágrafo, como o Word exige.
- **Estilo de tabela**: `PropsTabela::estilo` (`w:tblStyle`) e `aparencia` (os atributos do `w:tblLook`);
  `Estilos::de_tabela` guarda os `w:style w:type="table"` do `styles.xml` como vieram e o gravador os devolve. Com
  estilo, a grade cinza e o fundo do cabeçalho do FeatherDocs não entram por cima. No `.fdoz`, os dois campos da
  tabela vão no JSON `tabela` e os estilos nos metadados de `estilos.arrow` (`estilos_de_tabela`).
- **LaTeX**: a célula com quebra de linha sai num `\shortstack[l]{…}` (o `\\` terminava a linha da tabela).
- **Limites** (`// shortcut:` no código): a tela não desenha o estilo de tabela (fica a grade de sempre), nem o
  marcador do item dentro da célula (o texto aparece sem "•"); editar na tela uma célula com blocos a reduz a texto
  ao gravar; imagens e objetos da sacola dentro de célula vão no JSON do `.fdoz` (não em `media/`); o controle de
  conteúdo de bloco dentro de tabela continua fora da sacola.
- **Arquivos reais**: não rodados nesta parte (o corpus de 23 `.docx` fica fora do git).
- Teste: `docx::testes::celula_com_blocos_volta_e_a_edicao_vence` (`.docx`, `.fdoz` e a edição).

## Fase 5 — entregue (29/10/2026)

| Coluna | Antes `[x]` / `[~]` / `[ ]` | Depois |
|---|---|---|
| Lê | 147 / 31 / 31 | **162** / 25 / 22 |
| Grava | 156 / 20 / 33 | **171** / 14 / 24 |
| Volta igual | 178 / 0 / 31 | **187** / 0 / 22 |

Completos nas três colunas: 147 → **162**. §8 (21) e §9 (10) inteiras em `[x] [x] [x]`, e a marca d'água de §16.

- **Modelo** (`featherdocs-core/src/modelo.rs`): `Extras::mais: MaisPagina` (medianiz, distância do cabeçalho e
  do rodapé, colunas e o espaço entre elas, formato do número da página, passo da numeração de linhas, traço da
  borda, páginas pares diferentes com o cabeçalho e o rodapé delas). `Bloco::secao: Option<Box<Secao>>` na quebra
  de seção: o papel e as faixas da seção que termina ali (a de antes, como o `w:sectPr` do Word); `None` = os do
  documento. `Faixas::blocos` guarda o cabeçalho em blocos quando ele não cabe nas três posições (tabela, vários
  parágrafos); `Faixas::lida` é a `assinatura` de quando foi lido: mudada a faixa na tela, grava-se a faixa e os
  blocos não entram (`blocos_valem`). Já existiam e passaram a ser lidos e gravados: a letra e a imagem da faixa,
  o cabeçalho da primeira página, o número inicial (`pagina_inicial`) e a marca d'água.
- **Leitor**: o cabeçalho e o rodapé de cada tipo (`default`, `first`, `even`) e o `w:evenAndOddHeaders`; a faixa
  lê a letra do primeiro texto, a primeira imagem (pelas relações da própria parte) e, quando preciso, os blocos
  com o mesmo leitor do corpo. O `w:sectPr` dentro do parágrafo guarda o papel e as faixas na seção; a seção sem
  cabeçalho próprio herda o da de antes (como no Word); a igual à do documento não guarda nada (o documento do
  FeatherDocs volta sem seções a mais). A marca d'água sai do `v:textpath` do cabeçalho.
- **Gravador**: `header1`…`header3`/`footer1`…`footer3` (de sempre, primeira, pares) e `headerS<id>`/`footerS<id>`
  das seções; a faixa em blocos é gravada com um gravador só dela (as imagens vão para as relações da parte). A
  marca d'água leva o tipo de forma do WordArt (`v:shapetype` `_x0000_t136`): sem ele o LibreOffice não a desenha.
- **`.fdoz`**: colunas JSON opcionais `mais` e `faixas_blocos` em `documento.arrow` e `secao` em `blocos.arrow`.
  `tests/fdoz_antigo.rs` passa.
- **LibreOffice**: oito `.docx` gravados (seção em retrato e depois paisagem com cabeçalho próprio, letra do
  cabeçalho, tabela no rodapé, marca d'água, medianiz/colunas/numeração romana, pares, primeira página) abrem no
  `libreoffice --headless`; o PDF mostra o papel e o cabeçalho de cada seção e a marca d'água na diagonal. (O
  LibreOffice do snap não lê `/tmp`: converter de uma pasta da casa.)
- **Limites** (`// shortcut:` no código): a tela desenha o documento todo com o papel e as faixas do documento, sem
  colunas, medianiz, formato romano do número (fora do `{pagina_romano}`), passo da numeração de linhas, traço da
  borda nem o cabeçalho das pares; as faixas em blocos aparecem como texto nas três posições. Da seção, só o papel
  e o cabeçalho e o rodapé de sempre são próprios (primeira página, pares, colunas e o resto valem para o
  documento todo).
- **Arquivos reais**: não rodados nesta fase.
- Testes: `docx::testes::secoes_faixas_e_pagina_do_word_voltam` e `fdoz::testes::pagina_estilos_e_marcadores_sobrevivem_ao_arquivo`.

## Fase 6 — entregue (29/10/2026)

| Coluna | Antes `[x]` / `[~]` / `[ ]` | Depois |
|---|---|---|
| Lê | 162 / 25 / 22 | **173** / 18 / 18 |
| Grava | 171 / 14 / 24 | **178** / 12 / 19 |
| Volta igual | 187 / 0 / 22 | **191** / 0 / 18 |

Completos nas três colunas: 162 → **173**. §6 (9) inteira; em §7, a caixa de texto e os quatro controles de
conteúdo. Forma, gráfico, SmartArt e objeto OLE continuam na sacola (`[~]` em Lê de propósito: voltam iguais, não
são editados), como o plano previa.

- **Modelo** (`featherdocs-core/src/modelo.rs`): `Bloco::imagem: PropsImagem` — texto alternativo e título
  (`wp:docPr`), recorte (`a:srcRect`, em % de cada lado), rotação e espelhos (`a:xfrm`) e `flutuante: Option<Flutuante>`
  (o `wp:anchor`: posição horizontal e vertical, cada uma com o `relativeFrom` e o alinhamento ou o deslocamento,
  tipo e lado do contorno, atrás do texto, distâncias). `Marca::ObjetoWord::blocos`: o texto da caixa de texto em
  blocos; no XML guardado o lugar dele é `TEXTO_DA_CAIXA`. `featherdocs_core::controle::info` lê o `w:sdtPr` que o
  `Marca::Controle` guarda (tipo, título, marca, marcado, itens da lista, data e formato).
- **Leitor**: junta as propriedades durante o `w:drawing` e decide no fim dele: a imagem em linha num parágrafo com
  texto vira `Marca::Imagem` no lugar dela (antes saía do parágrafo para um bloco depois); a sozinha e a flutuante
  viram bloco de imagem com as propriedades. A imagem antiga (`w:pict`/`v:imagedata`) entra com o tamanho do
  estilo da forma. A caixa de texto (`w:txbxContent`) junta os parágrafos dela à parte, como a célula, e o
  `mc:Fallback` com a mesma caixa em VML é pulado (o texto entrava duas vezes).
- **Gravador**: `desenho_xml` grava `wp:inline` ou `wp:anchor` (com o polígono que o contorno justo e o
  atravessado pedem no esquema), `descr`/`title`, `a:srcRect` e `rot`/`flipH`/`flipV`; a imagem no meio do texto sai
  como `w:drawing` em linha na corrida; a caixa de texto volta com os blocos dela no lugar do `TEXTO_DA_CAIXA`.
- **`.fdoz`**: coluna JSON opcional `imagem` em `blocos.arrow`; o texto da caixa vai em JSON depois do resumo na
  coluna do objeto (separado por U+001E). `tests/fdoz_antigo.rs` passa.
- **LibreOffice**: um `.docx` com imagem flutuante (à direita da margem, girada 15°, espelhada, recortada, com o
  texto em volta), imagem no meio da linha e caixa de texto abre no `libreoffice --headless`, e o PDF mostra os três.
- **Tela**: a caixa de texto mostra o começo do texto dela (`[caixa de texto: …]`) no lugar do quadro vazio.
- **Limites** (`// shortcut:` no código): o motor desenha a imagem inteira, sem girar, e em linha (a flutuante fica
  no lugar do bloco); a imagem no meio do texto perde texto alternativo, recorte e rotação (a `Marca::Imagem` não
  tem onde guardar); o texto da caixa não se edita na tela; a caixa volta só com a versão nova (`mc:Choice`), sem a
  VML antiga do `mc:Fallback`; marcar a caixa de seleção ou escolher um item na tela não muda o `w:sdtPr`.
- **Arquivos reais**: não rodados nesta fase.
- Testes: `docx::testes::imagens_e_caixa_de_texto_do_word_voltam`, `controle::testes::le_os_quatro_tipos_de_controle`.

## Fase 7, primeira parte — entregue (29/10/2026)

| Coluna | Antes `[x]` / `[~]` / `[ ]` | Depois |
|---|---|---|
| Lê | 173 / 18 / 18 | **186** / 9 / 14 |
| Grava | 178 / 12 / 19 | **191** / 4 / 14 |
| Volta igual | 191 / 0 / 18 | **195** / 0 / 14 |

Completos nas três colunas: 173 → **186**. §10 (notas), §13 (campos) e §15 (fórmulas) inteiras; em §12 o texto movido;
em §14 o indicador, REF e PAGEREF.

- **Campos** (`featherdocs-core/src/campo.rs`): o campo que o FeatherDocs não calcula (`MERGEFIELD`, `REF`, `PAGEREF`,
  `=`, `IF`, `DOCPROPERTY`…) é um `Marca::Campo` com `campo` = `word:<instrução>␟<resultado>` (`campo::do_word`,
  `partes_do_word`): a tela mostra o resultado que o Word guardou e o `.docx` volta com a instrução. A data guarda o
  formato do Word (`data:d 'de' MMMM 'de' yyyy`; `data` = dd/MM/yyyy) e a tela a escreve nele (`formatar_data`, com
  os meses e os dias da semana em português). O `HYPERLINK` de campo vira link comum. O campo sem resultado também
  volta. Nada mudou no formato do `Marca::Campo` (o `.fdoz` já o guardava).
- **Indicadores**: o `w:bookmarkStart` entra em `Documento::marcadores` (o nome e o bloco do parágrafo em que começa;
  o `_GoBack` e os `_Toc` do sumário não). O gravador põe o indicador em volta do texto do primeiro parágrafo do bloco
  (é ele que o REF mostra).
- **Notas**: os parágrafos da nota ficam separados por `QUEBRA_DE_PARAGRAFO` (U+2029; a tela quebra a linha nele) e
  voltam como `w:p` próprios. O formato do número (`w:footnotePr`/`w:endnotePr` `w:numFmt` → `formato_notas`,
  `formato_notas_fim`) e o recomeço a cada página (`MaisPagina::notas_por_pagina`) são lidos e gravados.
- **Texto movido**: `Revisao::movido` (o `w:moveFrom` é a remoção, o `w:moveTo` a inserção); no `.fdoz`, os códigos 3
  e 4 da coluna `rev`. Aceitar e rejeitar na tela tratam como remoção e inserção.
- **Fórmula numerada**: o `SEQ Equação` não apaga mais a fórmula do parágrafo (só o texto comum, e o `)` depois do
  número).
- **Medição**: o `campo_simples` do exemplo escapa as aspas do `w:instr`, como o Word grava (antes o atributo saía
  malformado e o formato da data se perdia na leitura).
- **LibreOffice**: um `.docx` com MERGEFIELD, REF para um indicador, data com formato, nota com dois parágrafos em
  romano e texto movido abre no `libreoffice --headless`; o ODT tem o indicador, a referência (que mostra o texto
  marcado), o campo de banco de dados e as quatro marcas de alteração.
- **Limites** (`// shortcut:` no código): o resultado do campo guardado volta com a letra do primeiro pedaço; a tela
  não recalcula REF, PAGEREF nem `=` (mostra o resultado guardado); o indicador dentro de célula ou de caixa de texto
  vai para o próximo parágrafo do corpo.
- **Faltam** (segunda parte): iniciais do autor e formatação dentro do comentário; formatação, parágrafo, marca de
  parágrafo e linha de tabela alterados (§12, pedem marcas de revisão novas no modelo e em aceitar/rejeitar);
  entradas guardadas do sumário, legenda com rótulo próprio e a dica de tela do link.
- Testes: `docx::testes::campos_indicadores_notas_e_movidos_voltam`, `campo::testes::data_no_formato_do_word_e_campo_guardado`.

## Fase 7, segunda parte, e fase 8 — entregues (29/10/2026)

| Coluna | Antes `[x]` / `[~]` / `[ ]` | Depois |
|---|---|---|
| Lê | 186 / 9 / 14 | **197** / 7 / 5 |
| Grava | 191 / 4 / 14 | **201** / 3 / 5 |
| Volta igual | 195 / 0 / 14 | **204** / 0 / 5 |

Completos nas três colunas: 186 → **197 / 209**.

- **Iniciais do comentário**: `Comentario::iniciais` (o `w:initials`; vazio = as do nome, que o gravador refaz; as
  iguais às do nome voltam vazias). No `.fdoz`, coluna opcional `iniciais` em `comentarios.arrow`. A medição passou a
  usar iniciais que não saem do nome ("ANL"): com "AL", de "Ana Lima", o item passava sem guardar nada.
- **Legenda com rótulo próprio**: `Estilos::legendas` (os rótulos do `SEQ` que não são figura, tabela nem equação,
  na ordem em que aparecem); `Atrib::legenda` 4 é o primeiro deles. O motor numera cada rótulo à parte
  (`paginacao::numeros` recebe os próprios) e o gravador devolve o `SEQ Quadro`. No `.fdoz`, metadado `legendas` de
  `estilos.arrow`.
- **Propriedades do arquivo** (`Meta::mais: MaisMeta`): assunto, palavras-chave, descrição, criado e modificado
  (`core.xml`), empresa e gerente (`app.xml`) e as personalizadas (`custom.xml`, como texto). As partes `app.xml` e
  `custom.xml` só são gravadas quando há o que pôr. No `.fdoz`, coluna JSON opcional `meta` em `documento.arrow`.
- **Idioma**: `MaisLetra::idioma` (o `w:lang`, no catálogo de nomes da `Familia`, para o `Fmt` continuar `Copy`;
  `Conv::Nome` no mapa). O do documento é o do padrão (`w:docDefaults`); o pt-BR que o gravador põe quando o documento
  não diz outro não entra no padrão na volta.
- **Direção e hifenização**: `MaisLetra::outros` e `MaisParagrafo::outros` com `DIREITA_PARA_ESQUERDA` (`w:rtl`,
  `w:bidi`) e `SEM_HIFENIZACAO` (`w:suppressAutoHyphens`), pelo mapa; `MaisPagina::hifenizar` (`w:autoHyphenation`) e
  `tabulacao_padrao` (`w:defaultTabStop`; 0 = os 708 de sempre).
- **LibreOffice**: um `.docx` com essas propriedades, um trecho em hebraico da direita para a esquerda, a hifenização
  ligada no documento e desligada no parágrafo e a tabulação de 2 cm abre no `libreoffice --headless`, e o ODT tem
  tudo isso.
- **Limites** (`// shortcut:` no código): as propriedades do arquivo, a direção, a hifenização e a tabulação padrão só
  vão e voltam (a tela não mostra nem compõe da direita para a esquerda nem hifeniza); o idioma não desliga por
  herança de estilo; do `app.xml` só a empresa e o gerente.
- **Faltam** (5 itens em `[ ]`, 7 com `[~]` em Lê): formatação, parágrafo, marca de parágrafo e linha de tabela
  alterados (§12: pedem revisões novas no modelo e em aceitar/rejeitar); fontes embutidas (§16); formatação dentro
  do comentário (o comentário é texto puro); entradas guardadas do sumário e dica de tela do link (§14); forma,
  gráfico, SmartArt e OLE ficam na sacola de propósito (§7).
- Teste: `docx::testes::propriedades_idioma_e_direcao_voltam`.

## Fora deste plano (andam em paralelo)

- **O motor desenhar o que o modelo passou a guardar** (largura de coluna, nome da fonte,
  mescla vertical, seções com papel próprio): a ida e volta fica completa antes; a tela segue
  atrás, na ordem em que o usuário sentir falta.
- **Interface** para editar as propriedades novas (diálogos de estilo, de tabela, de seção).
- **"O Word abre sem reclamar"**: a coluna Grava confere o XML, não o Word. Para isso, abrir os
  `.docx` gravados no LibreOffice sem tela (`libreoffice --headless --convert-to pdf`, já instalado nesta máquina) e, de vez em
  quando, no Word do usuário.
- O `.doc` antigo (binário).

## Como cada fase termina

1. Os itens da fase em `[x] [x] [x]` no `paridade_docx` (ou `[~]` com o motivo escrito).
2. A catraca passando (nada piorou), e os testes de LaTeX, Markdown e `.fdoz` também.
3. `.fdoz` gravado antes da fase abre igual depois dela (teste com um arquivo da versão anterior).
4. O corpus de arquivos reais rodado, e o resultado anotado aqui.
5. `PARIDADE-DOCX.md` regenerado e commitado junto.

## Fase 9 — os 12 últimos (204 → 209 de 209)

- Formas, gráficos, SmartArt e OLE: `objeto_word::info` entende o XML guardado (tipo, nome, tamanho, geometria e cor da forma, tipo e séries do gráfico, programa do OLE); o XML continua voltando igual pela sacola.
- Alterações controladas: `TipoDeRevisao::Formatacao` com a formatação de antes (`w:rPrChange`); `Bloco::revisao` (`RevisoesDoBloco`) com o parágrafo de antes (`w:pPrChange`), a marca de parágrafo e as linhas da tabela inseridas ou apagadas. Rejeitar tudo volta a formatação e o parágrafo e tira as linhas inseridas.
- Comentário com negrito, itálico e sublinhado (`Comentario::trechos`, válido enquanto o texto for o mesmo).
- Entradas guardadas do sumário e fontes embutidas em `Extras::documento`; dica de tela do link em `MaisLetra::dica`.
- O corpo que termina em tabela ganha o parágrafo final que o Word exige (sem ele o LibreOffice travava).
