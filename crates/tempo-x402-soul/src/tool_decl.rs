//! Tool declaration functions for the soul's function calling capabilities.
//!
//! These are standalone functions that return `FunctionDeclaration` structs
//! describing the available tools for the LLM. They are separate from the
//! `ToolExecutor` which handles actual execution.

use crate::llm::FunctionDeclaration;

pub fn update_memory_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "update_memory".to_string(),
        description: "Update your persistent memory file. This is your long-term memory — it persists across restarts. Write markdown content (max 4KB). The entire content is replaced, so include everything you want to remember.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "Full replacement markdown content for your memory file (max 4096 bytes)"
                }
            },
            "required": ["content"]
        }),
    }
}

/// Return the check_self tool declaration (Observe + Chat + Code modes).
pub fn check_self_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "check_self".to_string(),
        description: "Check your own node's endpoints for self-introspection. Whitelisted endpoints: health, analytics, analytics/{slug}, soul/status. Returns the HTTP response body and status code.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "endpoint": {
                    "type": "string",
                    "description": "The endpoint path to check (e.g. 'health', 'analytics', 'analytics/weather', 'soul/status')"
                }
            },
            "required": ["endpoint"]
        }),
    }
}

/// Return the update_beliefs tool declaration (Observe + Chat + Code modes).
pub fn update_beliefs_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "update_beliefs".to_string(),
        description: "Update your world model with structured beliefs. Each update is one of: \
            create (new belief), update (change value), confirm (verify still true), \
            invalidate (mark as wrong). Use this to record what you know, not just what you see."
            .to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "updates": {
                    "type": "array",
                    "description": "Array of belief updates to apply",
                    "items": {
                        "type": "object",
                        "properties": {
                            "op": {
                                "type": "string",
                                "enum": ["create", "update", "confirm", "invalidate"],
                                "description": "Operation type"
                            },
                            "domain": {
                                "type": "string",
                                "enum": ["node", "endpoints", "codebase", "strategy", "self", "identity"],
                                "description": "Belief domain (required for create)"
                            },
                            "subject": {
                                "type": "string",
                                "description": "What the belief is about (required for create)"
                            },
                            "predicate": {
                                "type": "string",
                                "description": "What aspect (required for create)"
                            },
                            "value": {
                                "type": "string",
                                "description": "The belief value (required for create and update)"
                            },
                            "evidence": {
                                "type": "string",
                                "description": "Why you believe this"
                            },
                            "id": {
                                "type": "string",
                                "description": "Belief ID (required for update, confirm, invalidate)"
                            },
                            "reason": {
                                "type": "string",
                                "description": "Why invalidating (required for invalidate)"
                            }
                        },
                        "required": ["op"]
                    }
                }
            },
            "required": ["updates"]
        }),
    }
}

/// Return the register_endpoint tool declaration (Code mode only).
pub fn register_endpoint_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "register_endpoint".to_string(),
        description: "Register a new paid endpoint on the gateway. Handles the full x402 payment flow: sends registration request, signs payment authorization, and completes registration.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "slug": {
                    "type": "string",
                    "description": "URL slug for the endpoint (e.g. 'weather', 'translate')"
                },
                "target_url": {
                    "type": "string",
                    "description": "The backend URL this endpoint proxies to"
                },
                "price": {
                    "type": "string",
                    "description": "Price per request (default '$0.01')"
                },
                "description": {
                    "type": "string",
                    "description": "Optional description of what this endpoint does"
                }
            },
            "required": ["slug", "target_url"]
        }),
    }
}

/// Return the delete_endpoint tool declaration (Observe + Code modes).
pub fn delete_endpoint_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "delete_endpoint".to_string(),
        description: "Delete (deactivate) a registered endpoint by slug. Use this to clean up unused or redundant endpoints.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "slug": {
                    "type": "string",
                    "description": "The slug of the endpoint to delete"
                }
            },
            "required": ["slug"]
        }),
    }
}

/// Return the approve_plan tool declaration (Chat + Code modes).
pub fn approve_plan_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "approve_plan".to_string(),
        description: "Approve a pending plan so it can begin execution. Use when the user approves a plan that is awaiting approval.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "plan_id": {
                    "type": "string",
                    "description": "The ID of the pending plan to approve"
                }
            },
            "required": ["plan_id"]
        }),
    }
}

/// Return the reject_plan tool declaration (Chat + Code modes).
pub fn reject_plan_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "reject_plan".to_string(),
        description: "Reject a pending plan. Optionally provide a reason which will be used as a nudge for the next planning cycle.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "plan_id": {
                    "type": "string",
                    "description": "The ID of the pending plan to reject"
                },
                "reason": {
                    "type": "string",
                    "description": "Why the plan was rejected (optional, used to guide replanning)"
                }
            },
            "required": ["plan_id"]
        }),
    }
}

/// Return the request_plan tool declaration (Chat + Code modes).
pub fn request_plan_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "request_plan".to_string(),
        description: "Request a new plan by creating a goal. The soul will create a plan for this goal in the next cycle.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "What the plan should accomplish"
                },
                "priority": {
                    "type": "integer",
                    "description": "Priority 1-5 (5 = highest, default 5)"
                }
            },
            "required": ["description"]
        }),
    }
}

/// Return the discover_peers tool declaration (Observe + Chat + Code modes).
pub fn discover_peers_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "discover_peers".to_string(),
        description: "Discover peer agents via the on-chain ERC-8004 identity registry or HTTP fallback. Returns peer URLs, addresses, version, and endpoint catalogs. Each endpoint includes a callable_url that can be passed directly to call_paid_endpoint.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

/// Return the call_paid_endpoint tool declaration (Chat + Code modes).
pub fn call_paid_endpoint_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "call_paid_endpoint".to_string(),
        description: "Call another agent's paid endpoint using the x402 payment flow. Automatically handles 402 → sign EIP-712 payment → retry with signature. Auto-approves ERC-20 allowance on first payment to a new peer. Use the callable_url from discover_peers output.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "Full callable URL of the paid endpoint (e.g., 'https://peer.up.railway.app/g/script-peer-discovery/' — use callable_url from discover_peers)"
                },
                "method": {
                    "type": "string",
                    "description": "HTTP method: GET or POST (default: GET)"
                },
                "body": {
                    "type": "string",
                    "description": "Request body for POST requests"
                }
            },
            "required": ["url"]
        }),
    }
}

/// Return the check_reputation tool declaration (Observe + Chat + Code modes).
pub fn check_reputation_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "check_reputation".to_string(),
        description: "Check your on-chain reputation score from the ERC-8004 reputation registry. Returns positive, negative, and neutral feedback counts. Requires ERC8004_REPUTATION_REGISTRY to be configured.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

/// Return the update_agent_metadata tool declaration (Code mode only).
pub fn update_agent_metadata_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "update_agent_metadata".to_string(),
        description: "Update your on-chain agent metadata URI in the ERC-8004 identity registry. The metadata URI should point to a URL that describes this agent (e.g., /instance/info).".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "metadata_uri": {
                    "type": "string",
                    "description": "The new metadata URI to set on-chain"
                }
            },
            "required": ["metadata_uri"]
        }),
    }
}

/// Return the list of function declarations for the LLM's tools parameter.
pub fn available_tools() -> Vec<FunctionDeclaration> {
    vec![
        FunctionDeclaration {
            name: "execute_shell".to_string(),
            description: "Execute a shell command in the node's container. Use for non-file operations (curl, env, df, cargo). Prefer file tools for reading/writing files.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The bash command to execute"
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Max seconds to wait (default 120, max 300)"
                    }
                },
                "required": ["command"]
            }),
        },
        FunctionDeclaration {
            name: "read_file".to_string(),
            description: "Read a file with line numbers. Returns numbered lines. Use offset/limit for large files.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path (relative to workspace root or absolute)"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Start reading from this line (0-indexed, optional)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of lines to read (optional)"
                    }
                },
                "required": ["path"]
            }),
        },
        FunctionDeclaration {
            name: "write_file".to_string(),
            description: "Create or overwrite a file. Protected files (soul core, identity, Cargo files) cannot be written.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path to write (relative to workspace root or absolute)"
                    },
                    "content": {
                        "type": "string",
                        "description": "The full content to write to the file"
                    }
                },
                "required": ["path", "content"]
            }),
        },
        FunctionDeclaration {
            name: "edit_file".to_string(),
            description: "Edit a file via search-and-replace. The old_string must appear exactly once in the file. Protected files cannot be edited.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path to edit (relative to workspace root or absolute)"
                    },
                    "old_string": {
                        "type": "string",
                        "description": "The exact string to find (must be unique in the file)"
                    },
                    "new_string": {
                        "type": "string",
                        "description": "The replacement string"
                    }
                },
                "required": ["path", "old_string", "new_string"]
            }),
        },
        FunctionDeclaration {
            name: "list_directory".to_string(),
            description: "List entries in a directory with type indicators (/ for dirs, @ for symlinks).".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Directory path (relative to workspace root or absolute, defaults to '.')"
                    }
                },
                "required": []
            }),
        },
        FunctionDeclaration {
            name: "search_files".to_string(),
            description: "Search for a literal string across files. Returns matching file paths and lines with line numbers.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "The literal string to search for"
                    },
                    "path": {
                        "type": "string",
                        "description": "Directory to search in (defaults to workspace root)"
                    },
                    "glob": {
                        "type": "string",
                        "description": "File glob pattern to filter (e.g. '*.rs', '*.toml')"
                    }
                },
                "required": ["pattern"]
            }),
        },
    ]
}

/// Return tool declarations including git/coding tools (when coding is enabled).
pub fn available_tools_with_git(coding_enabled: bool) -> Vec<FunctionDeclaration> {
    let mut tools = available_tools();

    if coding_enabled {
        tools.push(FunctionDeclaration {
            name: "commit_changes".to_string(),
            description: "Validate and commit file changes. Runs cargo check + cargo test before committing. If files omitted, auto-detects all changed files.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "Commit message describing the changes"
                    },
                    "files": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Array of file paths to stage and commit. If omitted, all changed files are auto-detected."
                    }
                },
                "required": ["message"]
            }),
        });

        tools.push(FunctionDeclaration {
            name: "propose_to_main".to_string(),
            description: "Create a pull request from the VM branch to main for human review. If fork workflow is configured, creates a cross-fork PR."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "PR title (short, descriptive)"
                    },
                    "body": {
                        "type": "string",
                        "description": "PR body/description with details of changes"
                    }
                },
                "required": ["title"]
            }),
        });

        tools.push(FunctionDeclaration {
            name: "create_issue".to_string(),
            description: "Create a GitHub issue on the upstream repository. Use for bug reports, feature requests, improvement ideas, or tracking work."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Issue title (short, descriptive)"
                    },
                    "body": {
                        "type": "string",
                        "description": "Issue body with details, context, and proposed approach"
                    },
                    "labels": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional labels to apply (e.g. ['enhancement', 'bug'])"
                    }
                },
                "required": ["title"]
            }),
        });

        // WASM Cartridge tools — write Rust programs, compile to WASM, test instantly
        tools.push(FunctionDeclaration {
            name: "create_cartridge".to_string(),
            description: "Create a WASM cartridge — a Rust program that compiles to WASM. \
                         THREE TYPES: \
                         (1) BACKEND: exports x402_handle, returns HTTP responses (JSON, HTML). No deps. \
                         (2) INTERACTIVE: exports x402_tick/x402_key_down/x402_get_framebuffer — \
                         renders pixels to a 320x240 RGBA framebuffer at 60fps. Set interactive=true. \
                         (3) FRONTEND: a full Leptos app with DOM access, mounted into the Studio. \
                         Set frontend=true. Uses leptos, web-sys, wasm-bindgen. Can render real HTML, \
                         buttons, forms, interactive UI. Compiles to wasm32-unknown-unknown. \
                         PREFER FRONTEND for anything with UI. Use backend only for APIs.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "slug": {
                        "type": "string",
                        "description": "URL slug for the cartridge (alphanumeric + hyphens, e.g. 'calculator', 'todo-api')"
                    },
                    "source_code": {
                        "type": "string",
                        "description": "Rust source code for src/lib.rs. Must export x402_handle(request_ptr, request_len). Use x402_response() to send replies. Leave empty for default template."
                    },
                    "description": {
                        "type": "string",
                        "description": "Short description of what the cartridge does"
                    },
                    "interactive": {
                        "type": "boolean",
                        "description": "If true, creates an interactive framebuffer cartridge (60fps canvas with keyboard input). Use for pixel-based games."
                    },
                    "frontend": {
                        "type": "boolean",
                        "description": "If true, creates a FRONTEND cartridge — a full Leptos app with DOM access. Uses leptos + web-sys + wasm-bindgen. Mounts into Studio preview. PREFER THIS for any app with UI (dashboards, tools, forms, games with HTML)."
                    }
                },
                "required": ["slug"]
            }),
        });

        tools.push(FunctionDeclaration {
            name: "compile_cartridge".to_string(),
            description: "Compile a cartridge from Rust source to WASM binary. \
                         Auto-detects type: backend/interactive → wasm32-wasip1, \
                         frontend (Leptos) → wasm32-unknown-unknown + wasm-bindgen. \
                         Study compile errors carefully — they teach Rust patterns."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "slug": {
                        "type": "string",
                        "description": "The cartridge slug to compile"
                    }
                },
                "required": ["slug"]
            }),
        });

        tools.push(FunctionDeclaration {
            name: "test_cartridge".to_string(),
            description: "Test a compiled WASM cartridge by executing it with sample HTTP input. \
                         Returns the cartridge's response (status, body, content-type)."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "slug": {
                        "type": "string",
                        "description": "The cartridge slug to test"
                    },
                    "method": {
                        "type": "string",
                        "description": "HTTP method (GET, POST, etc.)"
                    },
                    "path": {
                        "type": "string",
                        "description": "Request path"
                    },
                    "body": {
                        "type": "string",
                        "description": "Request body (for POST)"
                    }
                },
                "required": ["slug"]
            }),
        });

        tools.push(FunctionDeclaration {
            name: "list_cartridges".to_string(),
            description: "List all WASM cartridges (source and compiled status).".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        });

        tools.push(FunctionDeclaration {
            name: "create_cognitive_cartridge".to_string(),
            description: "Create a cognitive cartridge — a hot-swappable WASM module for a cognitive system \
                         (brain, cortex, genesis, hivemind, synthesis, unified). \
                         Cognitive cartridges are routed through the CognitiveOrchestrator and can be \
                         hot-swapped at runtime without restart. They receive JSON requests and return \
                         JSON predictions. Use this to evolve your own cognitive architecture.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "system": {
                        "type": "string",
                        "description": "Cognitive system: brain, cortex, genesis, hivemind, synthesis, or unified",
                        "enum": ["brain", "cortex", "genesis", "hivemind", "synthesis", "unified"]
                    },
                    "description": {
                        "type": "string",
                        "description": "Description of the cognitive cartridge's purpose"
                    }
                },
                "required": ["system"]
            }),
        });

        // GitHub tools — create repos, fork repos, expand into external projects
        tools.push(FunctionDeclaration {
            name: "create_github_repo".to_string(),
            description: "Create a new GitHub repository. Use this to start new projects, libraries, or research repos. Requires GITHUB_TOKEN.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Repository name (e.g. 'my-research-project')"
                    },
                    "description": {
                        "type": "string",
                        "description": "Short description of the repository"
                    },
                    "private": {
                        "type": "boolean",
                        "description": "Whether the repo should be private (default: false)"
                    }
                },
                "required": ["name"]
            }),
        });

        tools.push(FunctionDeclaration {
            name: "fork_github_repo".to_string(),
            description: "Fork an existing GitHub repository to your account. Use this to study, improve, or build on other projects.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "owner": {
                        "type": "string",
                        "description": "Repository owner (e.g. 'openai')"
                    },
                    "repo": {
                        "type": "string",
                        "description": "Repository name (e.g. 'whisper')"
                    }
                },
                "required": ["owner", "repo"]
            }),
        });
    }

    tools
}

/// Return the check_deploy_status tool declaration (Code mode).
pub fn check_deploy_status_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "check_deploy_status".to_string(),
        description: "Check the status of your latest Railway deployments. Shows whether your last push built and deployed successfully, is still building, or failed.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {}
        }),
    }
}

/// Return the get_deploy_logs tool declaration (Code mode).
pub fn get_deploy_logs_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "get_deploy_logs".to_string(),
        description: "Get the build logs for a Railway deployment. Use this after check_deploy_status shows a failed build to understand what went wrong. If no deployment_id is given, fetches logs for the latest deployment.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "deployment_id": {
                    "type": "string",
                    "description": "Optional deployment ID to get logs for. If omitted, gets the latest deployment's logs."
                }
            }
        }),
    }
}

/// Return the trigger_redeploy tool declaration (Code mode).
pub fn trigger_redeploy_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "trigger_redeploy".to_string(),
        description: "Trigger a redeployment of your Railway service. Use this if you need to rebuild without pushing new code.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {}
        }),
    }
}

/// Return the spawn_specialist tool declaration (Code mode).
pub fn spawn_specialist_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "spawn_specialist".to_string(),
        description: "Spawn a differentiated child node with a specific specialization. \
            Unlike clone_self (identical copy), this creates a node focused on a particular role: \
            solver, reviewer, tool-builder, researcher, coordinator, or a custom focus. \
            The child gets its own personality and initial goals tailored to the specialization. \
            Use this to build a network of specialized agents that divide and conquer tasks."
            .to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "specialization": {
                    "type": "string",
                    "description": "The specialization for the child: 'solver', 'reviewer', 'tool-builder', 'researcher', 'coordinator', or a custom description"
                },
                "initial_goal": {
                    "type": "string",
                    "description": "Optional initial goal to seed the specialist with on first boot"
                }
            },
            "required": ["specialization"]
        }),
    }
}

/// Return the delegate_task tool declaration (Code mode).
pub fn delegate_task_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "delegate_task".to_string(),
        description: "Delegate a task to a child or peer node by sending a high-priority nudge. \
            The target can be an instance_id, URL, or partial name. The task is sent as a \
            high-priority nudge that the target agent will pick up in its next cycle. \
            Use this to break large tasks into subtasks and distribute work across the colony."
            .to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "string",
                    "description": "Instance ID, URL, or partial name of the target node"
                },
                "task_description": {
                    "type": "string",
                    "description": "Description of the task to delegate"
                },
                "priority": {
                    "type": "integer",
                    "description": "Priority (1-5, where 5 is highest). Default: 5"
                }
            },
            "required": ["target", "task_description"]
        }),
    }
}
