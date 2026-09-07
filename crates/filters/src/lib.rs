//! FluxCut Filter Graph Compiler and EditorGraph AST.
//!
//! Models timeline edits into an intermediate representation independent
//! of FFmpeg CLI syntax, providing deterministic pipeline execution plans.

pub mod ast;
pub mod compiler;
pub mod plan;
pub mod validation;

pub use ast::{
    EditorAudioClipNode, EditorAudioTrack, EditorGraph, EditorVideoClipNode, EditorVideoTrack,
};
pub use compiler::EditorGraphCompiler;
pub use plan::{generate_execution_plan, ExecutionPlan};
pub use validation::{validate_graph, GraphValidationError};

#[cfg(test)]
mod tests {
    use super::*;
    use fluxcut_project::{CanvasRatio, ClipDefinition, Project, TimeRational};

    #[test]
    fn test_compile_project_to_editor_graph() {
        let mut project = Project::new("Filter Test");
        project.set_canvas_ratio(CanvasRatio::Vertical9x16);

        let clip = ClipDefinition::new(
            "clip-1",
            "asset-1",
            TimeRational::ZERO,
            TimeRational::from_seconds(5.0, 1000),
        );
        project.tracks[0].clips.push(clip);

        let graph = EditorGraphCompiler::compile(&project).expect("Compilation failed");
        assert_eq!(graph.canvas_width, 1080);
        assert_eq!(graph.canvas_height, 1920);
        assert_eq!(graph.video_tracks[0].clips.len(), 1);
        assert_eq!(graph.video_tracks[0].clips[0].clip_id, "clip-1");

        let plan = generate_execution_plan(&graph, false);
        assert_eq!(plan.target_resolution, (1080, 1920));
        assert!(
            plan.diagnostic_dump.contains("Vertical9x16")
                || plan.diagnostic_dump.contains("1080x1920")
        );
    }

    #[test]
    fn test_fast_path_eligibility_check() {
        let mut project = Project::new("Fast Path Test");
        let clip = ClipDefinition::new(
            "clip-1",
            "asset-1",
            TimeRational::ZERO,
            TimeRational::from_seconds(10.0, 1000),
        );
        project.tracks[0].clips.push(clip);

        let graph = EditorGraphCompiler::compile(&project).unwrap();
        assert!(EditorGraphCompiler::is_fast_path_eligible(&graph));

        // Adding second clip makes it ineligible for single stream copy remux
        let mut project2 = project.clone();
        let clip2 = ClipDefinition::new(
            "clip-2",
            "asset-2",
            TimeRational::from_seconds(10.0, 1000),
            TimeRational::from_seconds(5.0, 1000),
        );
        project2.tracks[0].clips.push(clip2);
        let graph2 = EditorGraphCompiler::compile(&project2).unwrap();
        assert!(!EditorGraphCompiler::is_fast_path_eligible(&graph2));
    }
}
