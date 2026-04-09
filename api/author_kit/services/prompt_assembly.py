"""Assemble prose prompts (ported from Writingway muse/prompt_handler)."""

from __future__ import annotations

from typing import Any

from langchain_core.prompts import PromptTemplate


def assemble_final_prompt(
    prompt_config: dict[str, Any],
    user_input: str,
    additional_vars: dict[str, str] | None = None,
    current_scene_text: str | None = None,
    extra_context: str | None = None,
) -> PromptTemplate:
    prompt_text = prompt_config.get(
        "text", "Write a story chapter based on the following user input"
    )
    expected_vars = prompt_config.get("variables", [])

    base_template = """
    ### System
    {system_prompt}

    ### Context
    {context}

    ### Story Up-to-now
    {story_so_far}

    ### User
    {user_input}
    """

    if additional_vars:
        for var_name in additional_vars:
            base_template += f"\n### {var_name.capitalize()}\n{{{var_name}}}"

    default_vars = {
        "system_prompt": prompt_text,
        "context": extra_context or "No additional context provided.",
        "story_so_far": current_scene_text or "No previous story content.",
        "user_input": user_input,
    }
    if additional_vars:
        default_vars.update(additional_vars)

    prompt_template = PromptTemplate(
        input_variables=list(set(expected_vars + list(default_vars.keys()))),
        template=base_template,
    )

    missing_vars = [v for v in prompt_template.input_variables if v not in default_vars]
    if missing_vars:
        raise ValueError(f"Missing variables for prompt: {missing_vars}")

    return prompt_template.invoke(default_vars)


def preview_final_prompt(
    prompt_config: dict[str, Any],
    user_input: str,
    additional_vars: dict[str, str] | None = None,
    current_scene_text: str | None = None,
    extra_context: str | None = None,
) -> str:
    final_prompt = assemble_final_prompt(
        prompt_config,
        user_input,
        additional_vars,
        current_scene_text,
        extra_context,
    )
    return final_prompt.text
