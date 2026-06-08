---
title: Feedback and Training Data - Zed
description: Understand opt-in AI feedback ratings, Edit Prediction training data, and when Zed may retain AI data.
---

# Feedback and Training Data

Normal AI requests are not retained by Zed. For
[Zed-hosted models](../account/zed-hosted-models.md), provider agreements require
zero data retention and prohibit training on your prompts or code context. This
page covers the cases where Zed may retain AI data because you explicitly shared
it or opted in.

AI features in Zed include:

- [Agent Panel](./agent-panel.md)
- [Edit Prediction](./edit-prediction.md)
- [Inline Assistant](./inline-assistant.md)
- [Git commit generation](../git.md#ai-support-in-git)

For the broader request path and provider data boundaries, see
[AI Privacy](./privacy-and-security.md).

## Zed-Hosted Model Commitments {#data-retention-and-training}

Zed-hosted model zero-data-retention and no-training commitments are documented
on [AI Privacy](./privacy-and-security.md#data-retention-and-training).

## Response Ratings and Feedback {#ai-feedback-with-ratings}

You can rate AI responses or submit feedback to help improve Zed's system prompt,
tools, and AI product experience. Each share is opt-in, and sharing once does not
grant permission for future collection.

> **Warning:** Rating an AI response sends the conversation thread to Zed. The
> conversation thread includes your messages, AI responses, and thread metadata.
> If you do not want the thread persisted by Zed, do not rate the response.

### Data Collected from Feedback {#data-collected-ai-feedback}

For conversation threads you explicitly share through ratings or feedback, Zed
may store:

- your messages and AI responses in the conversation thread
- any commentary you include with your rating or feedback
- thread metadata, such as model used, token counts, and timestamps
- metadata about your Zed installation

If you do not rate responses or submit feedback, Zed does not store Customer Data
related to your AI feature usage for improvement.

Telemetry related to Zed's AI features is collected separately. This includes
metadata such as the AI feature being used and high-level interactions with the
feature to understand performance, such as agent response time or edit acceptance
and rejection. See [Telemetry](../telemetry.md) for details.

Collected feedback data is stored in Snowflake. Zed periodically reviews this
data to refine prompts, tools, and product behavior. Stored feedback data is
anonymized and stripped of sensitive information such as access tokens, user IDs,
and email addresses.

## Edit Prediction Training Data {#edit-predictions}

Zed does not collect training data for the Edit Prediction model unless all of
these conditions are met:

1. You opt in by toggling **Training Data Collection** under the **Privacy**
   section of the Edit Prediction status bar menu.
2. The project is open source, detected by the presence of a license file. See
   the [license detection logic](https://github.com/zed-industries/zed/blob/main/crates/edit_prediction/src/license_detection.rs).
3. The file is not excluded by `edit_predictions.disabled_globs`.

Edit Prediction setup and provider configuration live on the
[Edit Prediction](./edit-prediction.md) page. This page only covers training data
collection and retention.

### File Exclusions {#file-exclusions}

<<<<<<< HEAD
## Edit Predictions

Edit predictions can be powered by **Zed's Zeta model** or by **third-party providers** like GitHub Copilot.

### Zed's Zeta Model (Default)

Zed sends a limited context window to the model to generate predictions:

- A code excerpt around your cursor (not the full file)
- Recent edits as diffs
- Relevant excerpts from related open files

This data is processed transiently to generate predictions and is not retained afterward.

### Third-Party Providers

When using third-party providers like GitHub Copilot, **Zed does not control how your data is handled** by that provider. You should consult their Terms and Conditions directly.

Note: Zed's `disabled_globs` settings will prevent predictions from being requested, but third-party providers may receive file content when files are opened.

### Training Data: Opt-In for Open Source Projects

Zed does not collect training data for our edit prediction model unless the following conditions are met:

1. **You opt in** – Toggle "Training Data Collection" under the **Privacy** section of the edit prediction status bar menu (click the edit prediction icon in the status bar).
2. **The project is open source** — detected via LICENSE file ([see detection logic](https://github.com/zixiao-labs/Kaltsit-Esperanta/blob/main/crates/edit_prediction/src/license_detection.rs))
3. **The file isn't excluded** — via `disabled_globs`

### File Exclusions

Certain files are always excluded from edit predictions—regardless of opt-in status:
=======
Certain files are always excluded from Edit Prediction training data collection,
regardless of opt-in status:
>>>>>>> upstream/main

```json [settings]
{
  "edit_predictions": {
    "disabled_globs": [
      "**/.env*",
      "**/*.pem",
      "**/*.key",
      "**/*.cert",
      "**/*.crt",
      "**/.dev.vars",
      "**/secrets.yml"
    ]
  }
}
```

You can explicitly exclude additional paths or file extensions by adding them to
[`edit_predictions.disabled_globs`](https://zed.dev/docs/reference/all-settings#edit-predictions)
in your Zed settings file ([how to edit](../configuring-zed.md#settings-files)):

```json [settings]
{
  "edit_predictions": {
    "disabled_globs": ["secret_dir/*", "**/*.log"]
  }
}
```

### Data Collected from Edit Prediction Training {#data-collected-edit-prediction-training}

For open source projects where you opted in, Zed may collect:

- code excerpts around your cursor
- recent edit diffs
- the generated prediction
- repository URL and git revision
- buffer outline and diagnostics

Collected data is stored in Snowflake. Zed periodically reviews this data to
select training samples for inclusion in the model training dataset. Included
data is anonymized and stripped of sensitive information such as access tokens,
user IDs, and email addresses.

The training dataset is publicly available at
[huggingface.co/datasets/zed-industries/zeta](https://huggingface.co/datasets/zed-industries/zeta).

### Training Dataset and Model Output {#training-dataset-and-model-output}

Zed uses this training dataset to fine-tune
[Qwen2.5-Coder-7B](https://huggingface.co/Qwen/Qwen2.5-Coder-7B) and makes the
resulting model available at
[huggingface.co/zed-industries/zeta](https://huggingface.co/zed-industries/zeta).

## Business Controls {#business-controls}

On Zed Business, data sharing is off by default and controlled by organization
administrators. Administrators can prevent members from submitting agent thread
feedback or opting into Edit Prediction training data collection. See
[Privacy for Business](../business/privacy.md) and
[Admin Controls](../business/admin-controls.md#data-sharing).

## Applicable Terms {#applicable-terms}

See the [Zed Terms of Service](https://zed.dev/terms) for more.
