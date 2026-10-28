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
| 0 | Mapa único rPr/pPr, catraca, sacola de interoperabilidade | — | §7 e §16 passam a voltar iguais pela sacola |
| 1 | **Estilos**: estilos de parágrafo e caractere com nome, herança, `docDefaults`, Normal modificado, aparência dos títulos do `styles.xml`, Title/Subtitle; **nome da fonte** (e fonte do tema) | 1, 2, 8 | §3 inteira, fonte em §1 |
| 2 | **Caractere e parágrafo completos** pelo mapa: tipos e cor do sublinhado, tachado duplo, versalete, maiúsculas, espaçamento, escala, oculto; entrelinha exata/mínima, espaço automático, manter com o próximo, linhas juntas, órfãs, tabulações, nível de tópico, capitular | — | §1 e §2 |
| 3 | **Listas**: número inicial, reinício, continuar depois de parágrafo, texto do número ("Passo %1:"), marcador de símbolo, estilo de lista, numeração vinda do estilo (List Number) | 7 | §4 |
| 4 | **Tabelas**: célula com blocos, mescla horizontal lida, mescla vertical, larguras das colunas e da tabela, bordas da tabela e por célula, fundo, alinhamento vertical, altura da linha, margens, estilo de tabela, tabela aninhada | 3, 4, 9 | §5 |
| 5 | **Seções, cabeçalho e rodapé**: lista de seções (papel, margens, colunas, medianiz, distância do cabeçalho, número e formato da página), cabeçalho/rodapé em blocos, primeira página, pares/ímpares, um por seção, marca d'água | 6, 10 | §8, §9, marca d'água de §16 |
| 6 | **Imagens e objetos**: imagem no meio do texto, flutuante (âncora, posição, texto contornando), texto alternativo, recorte, rotação, VML antiga; caixa de texto de verdade; controles de conteúdo (texto, caixa, lista, data) | 5 | §6, §7 |
| 7 | **Notas, comentários, revisão, campos e referências**: vários parágrafos em nota e comentário, formato das notas, indicadores, REF/PAGEREF, DATE com formato, HYPERLINK de campo, MERGEFIELD, entradas guardadas do sumário, legenda com rótulo próprio; alterações de formatação, de parágrafo, movidas e de linha de tabela | — | §10 a §15 |
| 8 | **Documento**: propriedades (core, app, custom), idioma por trecho e padrão, direita-para-esquerda, hifenização, tabulação padrão, fontes embutidas | — | §16 |

Gráfico, SmartArt, formas e objetos OLE ficam na sacola (fase 0) até haver motivo para
editá-los; gráfico pode vir depois pelo `feather-charts`, que o FeatherSheet já usa.

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
