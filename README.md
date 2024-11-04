Talk to deepseek models from your terminal with small improvement from https://github.com/juanrgon/chatgpt-cli, just a toy.

## Feature
- streamed output
- multi-round chats
- supported-models
    - deepseek-chat


## Quickstart

First you'll need to install the CLI:

```
cargo install deepseek-cli
```

Then, you'll need to make sure your cargo bin directory is in your path. You can do this by adding the following to your `~/.bashrc` or `~/.zshrc`:

```
export PATH="$PATH:$HOME/.cargo/bin"
```

Finally, you'll need a DEEPSEEK API key (you can get one [here](https://platform.openai.com/account/api-keys)), and you'll need to export your API Key as an environment variable:


```
export DEEPSEEK_API=<your api key>
```


Then you can start a conversation with deepseek models:

```
deepseek-cli what is 2 + 2
```

## Todos
- docs
