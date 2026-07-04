//! Orquestrador (I/O) da deteccao de contexto de projeto. A logica pura (parse do titulo,
//! walk-up, precedencia, redacao, enquadramento) vive em `ember_core::project`; aqui ficam so
//! as leituras do filesystem. Best-effort: qualquer falha devolve `None` e o refine segue com o
//! perfil global (o comportamento de sempre). Default OFF na config.

use std::path::{Path, PathBuf};

use ember_core::project;

/// Resultado da deteccao, para o log/diagnostico (report honesto do que aconteceu).
pub struct ProjectContext {
    /// Bloco ja enquadrado e capado, pronto a injetar no system prompt.
    pub block: String,
    /// Caminho do ficheiro de contexto usado (para o report).
    pub source_path: String,
}

/// Resolve o bloco de contexto de projeto a partir do titulo da janela em foco. `None` quando
/// nao ha caminho no titulo, o diretorio nao existe, ou nao ha ficheiro de contexto por perto.
/// So le ficheiros de contexto CONHECIDOS (nunca .env, codigo, ou .git) e redige segredos.
pub fn resolve(title: &str, home: Option<&Path>) -> Option<ProjectContext> {
    let candidate = project::extract_path(title, home)?;
    let start = start_dir(&candidate)?;
    let exists = |p: &Path| p.exists();
    let is_git_root = |p: &Path| p.join(".git").exists();
    let found = project::nearest_context(&start, &exists, &is_git_root, home, false);
    let f = found.into_iter().next()?;
    let content = std::fs::read_to_string(&f.path).ok()?;
    let block = project::frame_project(&content)?;
    Some(ProjectContext {
        block,
        source_path: f.path.display().to_string(),
    })
}

/// O diretorio de onde comecar a subir: o candidato se for um diretorio, senao o pai (o titulo
/// costuma trazer o caminho de um FICHEIRO aberto, ex.: `.../src/main.rs`).
fn start_dir(candidate: &Path) -> Option<PathBuf> {
    if candidate.is_dir() {
        return Some(candidate.to_path_buf());
    }
    candidate
        .parent()
        .filter(|p| p.is_dir())
        .map(Path::to_path_buf)
}
