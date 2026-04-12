# Workflows

This directory contains workflow definitions for the accounting module.

## What are Workflows?

Workflows define multi-step business processes that span multiple entities or require
saga-like compensation logic.

## Example Workflow

```yaml
# example.workflow.yaml
name: ExampleWorkflow
description: Example multi-step workflow

trigger:
  event: ExampleCreatedEvent

steps:
  - name: validate
    type: action
    action: validate_example
    on_success:
      next: process

  - name: process
    type: action
    action: process_example
    on_success:
      next: notify
    on_failure:
      next: compensate

  - name: notify
    type: action
    action: send_notification
    terminal: true

  - name: compensate
    type: compensation
    action: rollback_example
    terminal: true

config:
  timeout: 30.minutes
  retry:
    max_attempts: 3
    backoff: exponential
```

## Running Schema Generation

After adding workflow files, run:

```bash
backbone schema generate accounting --target flow
```
