import { createSignal, createEffect, onMount, onCleanup } from 'solid-js';
import init, { convert } from '../../crates/parser/pkg/parser';

import { EditorState } from '@codemirror/state';
import { EditorView, keymap, highlightActiveLine } from '@codemirror/view';
import { markdown } from '@codemirror/lang-markdown';
import { oneDark } from '@codemirror/theme-one-dark';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';

import Prism from 'prismjs';
import 'prismjs/components/prism-lua';
import 'prismjs/themes/prism-tomorrow.css';

const DEFAULT_MARKDOWN = `# Eon1-67 Update [ 06.07.2067 ]

## New Contents

### [ 67 Auras ]

\`- 30 TRANSCENDENT tier auras\`
\`- 37 GLORIOUS tier auras\`

## Dev Note

\`This week, we did 6-7.\`

\`In addition, we did 6-1 too.\`

\`Thank you\`
\`- 54_xyz -\``

export default function Converter() {
    const [markdownText, setMarkdownText] = createSignal(DEFAULT_MARKDOWN);
    const [luauText, setLuauText] = createSignal('');
    const [isReady, setIsReady] = createSignal(false);

    let editorContainer: HTMLDivElement | undefined;
    let editorView: EditorView | undefined;

    onMount(async () => {
        await init();
        setIsReady(true);

        if (editorContainer) {
            const state = EditorState.create({
                doc: markdownText(),
                extensions: [
                    // lineNumbers(),
                    highlightActiveLine(),
                    history(),
                    keymap.of([...defaultKeymap, ...historyKeymap]),
                    markdown(),
                    oneDark,
                    EditorView.lineWrapping,
                    EditorView.updateListener.of((update) => {
                        if (update.docChanged) {
                            setMarkdownText(update.state.doc.toString());
                        }
                    }),
                    EditorView.theme({
                        '&': { height: '100%', 'font-size': '14px' },
                        '.cm-scroller': { 'font-family': 'var(--code-font)', 'scrollbar-width': 'none', },
                        '.cm-scroller::-webkit-scrollbar': { display: 'none' },
                        '.cm-content': { padding: '0.75rem 0' },
                    }),
                ],
            });

            editorView = new EditorView({
                state,
                parent: editorContainer,
            });
        }
    });

    onCleanup(() => {
        editorView?.destroy();
    });

    createEffect(() => {
        if (isReady()) {
            setLuauText(convert(markdownText()));
        }
    });

    const highlightedLuau = () => {
        if (!isReady()) return '';
        return Prism.highlight(luauText(), Prism.languages.lua, 'lua');
    };

    return (
        <div class="io-grid">
            <section class="panel panel--in">
                <div class="panel-header">
                    <span class="arw">◀</span>
                    <span class="ph-label">Markdown 입력</span>
                    <span class="arw">▶</span>
                </div>
                <div class="panel-body">
                    <div ref={ editorContainer } class="cm-host" />
                </div>
            </section>

            <section class="panel panel--out">
                <div class="panel-header">
                    <span class="arw">◀</span>
                    <span class="ph-label">Luau 출력</span>
                    <span class="arw">▶</span>
                </div>
                <div class="panel-body">
                    {isReady() ? (
                        <pre class="luau-output">
                            <code class="language-lua" innerHTML={ highlightedLuau() } />
                        </pre>
                    ) : (
                        <div class="luau-loading">Loading WASM...</div>
                    )}
                </div>
            </section>
        </div>
    );
}