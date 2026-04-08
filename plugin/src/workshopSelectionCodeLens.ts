import * as vscode from 'vscode';
import { getWorkspaceRoot } from './config';

const CFG_SELECTION_CODELENS = 'selectionCodeLens';

/**
 * CodeLens on the first line of a non-empty selection — closest VS Code can get to Cursor’s
 * inline “add to chat” (no floating overlay API for extensions).
 */
export function registerWorkshopSelectionCodeLens(context: vscode.ExtensionContext): void {
  const provider = new WorkshopSelectionCodeLensProvider();

  const refresh = (): void => {
    provider.refresh();
  };

  const syncActive = (): void => {
    const ed = vscode.window.activeTextEditor;
    if (ed) {
      provider.setFromEditor(ed);
    } else {
      provider.clear();
    }
  };

  context.subscriptions.push(
    vscode.languages.registerCodeLensProvider([{ scheme: 'file' }, { scheme: 'untitled' }], provider),
    vscode.window.onDidChangeTextEditorSelection((e) => {
      provider.setFromEditor(e.textEditor);
    }),
    vscode.window.onDidChangeActiveTextEditor(() => {
      syncActive();
    }),
    vscode.workspace.onDidChangeConfiguration((ev) => {
      if (ev.affectsConfiguration(`authorkit.${CFG_SELECTION_CODELENS}`)) {
        refresh();
      }
    })
  );

  syncActive();
}

class WorkshopSelectionCodeLensProvider implements vscode.CodeLensProvider {
  private readonly _onDidChange = new vscode.EventEmitter<void>();
  readonly onDidChangeCodeLenses = this._onDidChange.event;

  /** Editor that last had a non-empty selection update. */
  private trackedEditor: vscode.TextEditor | undefined;
  private primarySelection: vscode.Selection | undefined;

  setFromEditor(editor: vscode.TextEditor): void {
    this.trackedEditor = editor;
    const sel = editor.selection;
    this.primarySelection = sel.isEmpty ? undefined : sel;
    this._onDidChange.fire();
  }

  clear(): void {
    this.trackedEditor = undefined;
    this.primarySelection = undefined;
    this._onDidChange.fire();
  }

  refresh(): void {
    this._onDidChange.fire();
  }

  provideCodeLenses(
    document: vscode.TextDocument,
    _token: vscode.CancellationToken
  ): vscode.ProviderResult<vscode.CodeLens[]> {
    if (!getWorkspaceRoot()) {
      return [];
    }
    const enabled = vscode.workspace.getConfiguration('authorkit').get<boolean>(CFG_SELECTION_CODELENS);
    if (enabled === false) {
      return [];
    }

    const ed = this.trackedEditor;
    const sel = this.primarySelection;
    if (!ed || !sel || ed.document !== document) {
      return [];
    }

    const startLine = sel.start.line;
    const range = new vscode.Range(startLine, 0, startLine, 0);
    const title = vscode.l10n.t('Workshop: add selection');
    const lens = new vscode.CodeLens(range, {
      title: `$(selection) ${title}`,
      tooltip: vscode.l10n.t('Add the current selection as context for the AuthorKit Workshop.'),
      command: 'authorkit.addSelectionToWorkshop',
    });
    return [lens];
  }
}
