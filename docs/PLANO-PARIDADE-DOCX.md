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
| 3 | **Listas**: número inicial, reinício, continuar depois de parágrafo, texto do número ("Passo %1:"), marcador de símbolo, estilo de lista, numeração vinda do estilo (List Number) | 7 | §4 |
| 4 | **Tabelas**: célula com blocos, mescla horizontal lida, mescla vertical, larguras das colunas e da tabela, bordas da tabela e por célula, fundo, alinhamento vertical, altura da linha, margens, estilo de tabela, tabela aninhada | 3, 4, 9 | §5 |
| 5 | **Seções, cabeçalho e rodapé**: lista de seções (papel, margens, colunas, medianiz, distância do cabeçalho, número e formato da página), cabeçalho/rodapé em blocos, primeira página, pares/ímpares, um por seção, marca d'água | 6, 10 | §8, §9, marca d'água de §16 |
| 6 | **Imagens e objetos**: imagem no meio do texto, flutuante (âncora, posição, texto contornando), texto alternativo, recorte, rotação, VML antiga; caixa de texto de verdade; controles de conteúdo (texto, caixa, lista, data) | 5 | §6, §7 |
| 7 | **Notas, comentários, revisão, campos e referências**: vários parágrafos em nota e comentário, formato das notas, indicadores, REF/PAGEREF, DATE com formato, HYPERLINK de campo, MERGEFIELD, entradas guardadas do sumário, legenda com rótulo próprio; alterações de formatação, de parágrafo, movidas e de linha de tabela | — | §10 a §15 |
| 8 | **Documento**: propriedades (core, app, custom), idioma por trecho e padrão, direita-para-esquerda, hifenização, tabulação padrão, fontes embutidas | — | §16 |

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
