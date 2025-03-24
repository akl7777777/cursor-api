# cursor-api

## 说明

* 当前版本已稳定，若发现响应出现缺字漏字，与本程序无关。
* 若发现首字慢，与本程序无关。
* 若发现响应出现乱码，也与本程序无关。
* 属于官方的问题，请不要像作者反馈。
* 本程序拥有堪比客户端原本的速度，甚至可能更快。
* 本程序的性能是非常厉害的。
* 根据本项目开源协议，Fork的项目不能以作者的名义进行任何形式的宣传、推广或声明。

## 获取key

1. 访问 [www.cursor.com](https://www.cursor.com) 并完成注册登录
2. 在浏览器中打开开发者工具（F12）
3. 在 Application-Cookies 中查找名为 `WorkosCursorSessionToken` 的条目，并复制其第三个字段。请注意，%3A%3A 是 :: 的 URL 编码形式，cookie 的值使用冒号 (:) 进行分隔。

## 配置说明

### 环境变量

* `PORT`: 服务器端口号（默认：3000）
* `AUTH_TOKEN`: 认证令牌（必须，用于API认证）
* `ROUTE_PREFIX`: 路由前缀（可选）
* `TOKEN_LIST_FILE`: token列表文件路径（默认：.tokens）

更多请查看 `/env-example`

### Token文件格式

`.tokens` 文件：每行为token和checksum的对应关系：
    
```
# 这里的#表示这行在下次读取要删除
token1,checksum1
token2,checksum2
```

该文件可以被自动管理，但用户仅可在确认自己拥有修改能力时修改，一般仅有以下情况需要手动修改：

* 需要删除某个 token
* 需要使用已有 checksum 来对应某一个 token

### 模型列表

写死了，后续也不会会支持自定义模型列表

```
claude-3.5-sonnet
gpt-4
gpt-4o
claude-3-opus
cursor-fast
cursor-small
gpt-3.5-turbo
gpt-4-turbo-2024-04-09
gpt-4o-128k
gemini-1.5-flash-500k
claude-3-haiku-200k
claude-3-5-sonnet-200k
gpt-4o-mini
o1-mini
o1-preview
o1
claude-3.5-haiku
gemini-2.0-pro-exp
gemini-2.0-flash-thinking-exp
gemini-2.0-flash
deepseek-v3
deepseek-r1
o3-mini
grok-2
```

## 接口说明

### 基础对话

* 接口地址: `/v1/chat/completions`
* 请求方法: POST
* 认证方式: Bearer Token
  1. 使用环境变量 `AUTH_TOKEN` 进行认证
  2. 使用 `.token` 文件中的令牌列表进行轮询认证
  3. 在v0.1.3-rc.3支持直接使用 token,checksum 进行认证，但未提供配置关闭

#### 请求格式

```json
{
  "model": "string",
  "messages": [
    {
      "role": "system" | "user" | "assistant", // 也可以是 "developer" | "human" | "ai"
      "content": "string" | [
        {
          "type": "text" | "image_url",
          "text": "string",
          "image_url": {
            "url": "string"
          }
        }
      ]
    }
  ],
  "stream": boolean
}
```

#### 响应格式

如果 `stream` 为 `false`:

```json
{
  "id": "string",
  "object": "chat.completion",
  "created": number,
  "model": "string",
  "choices": [
    {
      "index": number,
      "message": {
        "role": "assistant",
        "content": "string"
      },
      "finish_reason": "stop" | "length"
    }
  ],
  "usage": {
    "prompt_tokens": 0,
    "completion_tokens": 0,
    "total_tokens": 0
  }
}
```

不进行 tokens 计算主要是担心性能问题。

如果 `stream` 为 `true`:

```
data: {"id":"string","object":"chat.completion.chunk","created":number,"model":"string","choices":[{"index":number,"delta":{"role":"assistant","content":"string"},"finish_reason":null}]}

data: {"id":"string","object":"chat.completion.chunk","created":number,"model":"string","choices":[{"index":number,"delta":{"content":"string"},"finish_reason":null}]}

data: {"id":"string","object":"chat.completion.chunk","created":number,"model":"string","choices":[{"index":number,"delta":{},"finish_reason":"stop"}]}

data: [DONE]
```

### 获取模型列表

* 接口地址: `/v1/models`
* 请求方法: GET
* 认证方式: Bearer Token

#### 响应格式

```json
{
  "object": "list",
  "data": [
    {
      "id": "string",
      "object": "model",
      "created": number,
      "owned_by": "string"
    }
  ]
}
```

#### 更新模型列表说明

每次携带Token时都会拉取最新的模型列表，与上次更新需距离至少30分钟。

### Token管理接口

#### 简易Token信息管理页面

* 接口地址: `/tokens`
* 请求方法: GET
* 响应格式: HTML页面
* 功能: 调用下面的各种相关API的示例页面


#### 获取Token信息

* 接口地址: `/tokens/get`
* 请求方法: POST
* 认证方式: Bearer Token
* 响应格式:

```json
{
  "status": "success",
  "tokens": [
    {
      "token": "string",
      "checksum": "string",
      "profile": { // 可能存在
        "usage": {
          "premium": {
            "requests": number,
            "requests_total": number,
            "tokens": number,
            "max_requests": number,
            "max_tokens": number
          },
          "standard": {
            "requests": number,
            "requests_total": number,
            "tokens": number,
            "max_requests": number,
            "max_tokens": number
          },
          "unknown": {
            "requests": number,
            "requests_total": number,
            "tokens": number,
            "max_requests": number,
            "max_tokens": number
          },
          "start_of_month": "string"
        },
        "user": {
          "email": "string",
          "name": "string",
          "id": "string",
          "updated_at": "string"
        },
        "stripe": {
          "membership_type": "free" | "free_trial" | "pro" | "enterprise",
          "payment_id": "string",
          "days_remaining_on_trial": number
        }
      }
    }
  ],
  "tokens_count": number
}
```

#### 重载Token信息

* 接口地址: `/tokens/reload`
* 请求方法: POST
* 认证方式: Bearer Token
* 响应格式:

```json
{
  "status": "success",
  "tokens_count": number,
  "message": "Token list has been reloaded"
}
```

#### 更新Token信息

* 接口地址: `/tokens/update`
* 请求方法: POST
* 认证方式: Bearer Token
* 请求格式:

```json
{
  "tokens": "string"  // token列表内容，将会直接覆盖 token_list 文件
}
```

* 响应格式:

```json
{
  "status": "success",
  "tokens_count": number,
  "message": "Token files have been updated and reloaded"
}
```

#### 添加Token

* 接口地址: `/tokens/add`
* 请求方法: POST
* 认证方式: Bearer Token
* 请求格式:

```json
{
  "tokens": [
    {
      "token": "string",
      "checksum": "string"  // 可选，如果不提供将自动生成
    }
  ],
  "tags": ["string"]
}
```

* 响应格式:

```json
{
  "status": "success",
  "tokens_count": number,
  "message": "string"  // "New tokens have been added and reloaded" 或 "No new tokens were added"
}
```

#### 删除Token

* 接口地址: `/tokens/delete`
* 请求方法: POST
* 认证方式: Bearer Token
* 请求格式:

```json
{
  "tokens": ["string"],  // 要删除的token列表
  "expectation": "simple" | "updated_tokens" | "failed_tokens" | "detailed" // 默认为simple
}
```

* 响应格式:

```json
{
  "status": "success",
  "updated_tokens": ["string"],  // 可选，根据expectation返回，表示更新后的token列表
  "failed_tokens": ["string"]    // 可选，根据expectation返回，表示未找到的token列表
}
```

* expectation说明:
  - simple: 只返回基本状态
  - updated_tokens: 返回更新后的token列表
  - failed_tokens: 返回未找到的token列表
  - detailed: 返回完整信息（包括updated_tokens和failed_tokens）

#### 更新Tokens标签

* 接口地址: `/tokens/tags/update`
* 请求方法: POST
* 认证方式: Bearer Token
* 请求格式:

```json
{
  "tokens": ["string"],
  "tags": ["string"]
}
```

* 响应格式:

```json
{
  "status": "success",
  "message": "string"  // "标签更新成功"
}
```

#### 构建API Key

* 接口地址: `/build-key`
* 请求方法: POST
* 认证方式: Bearer Token (当SHARE_AUTH_TOKEN启用时需要)
* 请求格式:

```json
{
  "auth_token": "string",  // 格式: {token},{checksum}
  "disable_vision": boolean,       // 可选，禁用图片处理能力
  "enable_slow_pool": boolean,     // 可选，启用慢速池
  "usage_check_models": {          // 可选，使用量检查模型配置
    "type": "default" | "disabled" | "all" | "custom",
    "model_ids": "string"  // 当type为custom时生效，以逗号分隔的模型ID列表
  },
  "include_web_references": boolean
}
```

* 响应格式:

```json
{
  "key": "string"    // 成功时返回生成的key
}
```

或出错时:

```json
{
  "error": "string"  // 错误信息
}
```

说明：

1. 此接口用于生成携带动态配置的API Key，是对直接传token与checksum模式的升级版本

2. API Key特性对比：

| 优势 | 劣势 |
|------|------|
| 提取关键信息，生成更短的密钥 | 可能存在版本兼容性问题 |
| 支持携带自定义配置 | 增加了程序复杂度 |
| 采用非常规编码方式，提升安全性 | 项目是开源的，安全性的提升相当于没有 |
| 更容易验证Key的合法性 | |
| 取消预校验带来轻微性能提升 | |

3. 生成的key格式为: `sk-{encoded_config}`，其中sk-为默认前缀(可配置)

4. auth_token的格式为: `{token},{checksum}`，其中,为默认分隔符(可配置)

5. usage_check_models配置说明:
   - default: 使用默认模型列表(同下 `usage_check_models` 字段的默认值)
   - disabled: 禁用使用量检查
   - all: 检查所有可用模型
   - custom: 使用自定义模型列表(需在model_ids中指定)

### 配置管理接口

#### 配置页面

* 接口地址: `/config`
* 请求方法: GET
* 响应格式: HTML页面
* 功能: 提供配置管理界面,可以修改页面内容和系统配置

#### 更新配置

* 接口地址: `/config`
* 请求方法: POST
* 认证方式: Bearer Token
* 请求格式:

```json
{
  "action": "get" | "update" | "reset",
  "path": "string",
  "content": {
    "type": "default" | "text" | "html",
    "content": "string"
  },
  "vision_ability": "none" | "base64" | "all", // "disabled" | "base64-only" | "base64-http"
  "enable_slow_pool": boolean,
  "enable_all_claude": boolean,
  "usage_check_models": {
    "type": "none" | "default" | "all" | "list",
    "content": "string"
  },
  "enable_dynamic_key": boolean,
  "share_token": "string",
  "proxies": "" | "system" | "proxy1,proxy2,...",
  "include_web_references": boolean
}
```

* 响应格式:

```json
{
  "status": "success",
  "message": "string",
  "data": {
    "page_content": {
      "type": "default" | "text" | "html", // 对于js和css后两者是一样的
      "content": "string"
    },
    "vision_ability": "none" | "base64" | "all",
    "enable_slow_pool": boolean,
    "enable_all_claude": boolean,
    "usage_check_models": {
      "type": "none" | "default" | "all" | "list",
      "content": "string"
    },
    "enable_dynamic_key": boolean,
    "share_token": "string",
    "proxies": "" | "system" | "proxy1,proxy2,...",
    "include_web_references": boolean
  }
}
```

注意：`usage_check_models` 字段的默认值为：

```json
{
  "type": "default",
  "content": "claude-3-5-sonnet-20241022,claude-3.5-sonnet,gemini-exp-1206,gpt-4,gpt-4-turbo-2024-04-09,gpt-4o,claude-3.5-haiku,gpt-4o-128k,gemini-1.5-flash-500k,claude-3-haiku-200k,claude-3-5-sonnet-200k"
}
```

这些模型将默认进行使用量检查。您可以通过配置接口修改此设置。

路径修改注意：选择类型再修改文本，否则选择默认时内容的修改无效，在更新配置后自动被覆盖导致内容丢失，自行改进。

### 静态资源接口

#### 获取共享样式

* 接口地址: `/static/shared-styles.css`
* 请求方法: GET
* 响应格式: CSS文件
* 功能: 获取共享样式表

#### 获取共享脚本

* 接口地址: `/static/shared.js`
* 请求方法: GET
* 响应格式: JavaScript文件
* 功能: 获取共享JavaScript代码

#### 环境变量示例

* 接口地址: `/env-example`
* 请求方法: GET
* 响应格式: 文本文件
* 功能: 获取环境变量配置示例

### 其他接口

#### 获取一个随机hash

* 接口地址: `/get-hash`
* 请求方法: GET
* 响应格式:

```plaintext
string
```

#### 获取或修复checksum

* 接口地址: `/get-checksum`
* 请求方法: GET
* 请求参数:
  * `checksum`: 可选，用于修复的旧版本生成的checksum，也可只传入前8个字符；可用来自动刷新时间戳头
* 响应格式:

```plaintext
string
```

说明：

* 如果不提供`checksum`参数，将生成一个新的随机checksum
* 如果提供`checksum`参数，将尝试修复旧版本的checksum以适配v0.1.3-rc.3之后的版本使用，修复失败会返回新的checksum；若输入的checksum本来就有效，则返回更新tsheader后的checksum

#### 获取当前的tsheader

* 接口地址: `/get-tsheader`
* 请求方法: GET
* 响应格式:

```plaintext
string
```

#### 健康检查接口

* 接口地址: `/health` 或 `/`(重定向)
* 请求方法: GET
* 认证方式: Bearer Token（可选）
* 响应格式: 根据配置返回不同的内容类型(默认、文本或HTML)，默认JSON

```json
{
  "status": "success",
  "version": "string",
  "uptime": number,
  "stats": {
    "started": "string",
    "total_requests": number,
    "active_requests": number,
    "system": {
      "memory": {
        "rss": number
      },
      "cpu": {
        "usage": number
      }
    }
  },
  "models": ["string"],
  "endpoints": ["string"]
}
```

注意：`stats` 字段仅在请求头中包含正确的 `AUTH_TOKEN` 时才会返回。否则，该字段将被省略。

#### 获取日志接口

* 接口地址: `/logs`
* 请求方法: GET
* 响应格式: 根据配置返回不同的内容类型(默认、文本或HTML)

#### 获取日志数据

* 接口地址: `/logs`
* 请求方法: POST
* 认证方式: Bearer Token
* 响应格式:

```json
{
  "total": number,
  "logs": [
    {
      "id": number,
      "timestamp": "string",
      "model": "string",
      "token_info": {
        "token": "string",
        "checksum": "string",
        "profile": {
          "usage": {
            "premium": {
              "requests": number,
              "requests_total": number,
              "tokens": number,
              "max_requests": number,
              "max_tokens": number
            },
            "standard": {
              "requests": number,
              "requests_total": number,
              "tokens": number,
              "max_requests": number,
              "max_tokens": number
            },
            "unknown": {
              "requests": number,
              "requests_total": number,
              "tokens": number,
              "max_requests": number,
              "max_tokens": number
            },
            "start_of_month": "string"
          },
          "user": {
            "email": "string",
            "name": "string",
            "id": "string",
            "updated_at": "string"
          },
          "stripe": {
            "membership_type": "free" | "free_trial" | "pro" | "enterprise",
            "payment_id": "string",
            "days_remaining_on_trial": number
          }
        }
      },
      "prompt": "string",
      "timing": {
        "total": number,
        "first": number
      },
      "stream": boolean,
      "status": "string",
      "error": "string"
    }
  ],
  "timestamp": "string",
  "status": "success"
}
```

#### 获取用户信息

* 接口地址: `/userinfo`
* 请求方法: POST
* 认证方式: 请求体中包含token
* 请求格式:

```json
{
  "token": "string"
}
```

* 响应格式:

```json
{
  "usage": {
    "premium": {
      "requests": number,
      "requests_total": number,
      "tokens": number,
      "max_requests": number,
      "max_tokens": number
    },
    "standard": {
      "requests": number,
      "requests_total": number,
      "tokens": number,
      "max_requests": number,
      "max_tokens": number
    },
    "unknown": {
      "requests": number,
      "requests_total": number,
      "tokens": number,
      "max_requests": number,
      "max_tokens": number
    },
    "start_of_month": "string"
  },
  "user": {
    "email": "string",
    "name": "string",
    "id": "string",
    "updated_at": "string"
  },
  "stripe": {
    "membership_type": "free" | "free_trial" | "pro" | "enterprise",
    "payment_id": "string",
    "days_remaining_on_trial": number
  }
}
```

如果发生错误，响应格式为:

```json
{
  "error": "string"
}
```

#### 基础校准

* 接口地址: `/basic-calibration`
* 请求方法: POST
* 认证方式: 请求体中包含token
* 请求格式:

```json
{
  "token": "string"
}
```

* 响应格式:

```json
{
  "status": "success" | "error",
  "message": "string",
  "user_id": "string",
  "create_at": "string",
  "checksum_time": number
}
```

注意: `user_id`, `create_at`, 和 `checksum_time` 字段在校验失败时可能不存在。

## 项目相关工具

### 获取token

- 使用 [get-token](https://github.com/wisdgod/cursor-api/tree/main/tools/get-token) 获取读取当前用户token，仅支持windows、linux与macos

### 重置遥测数据

- 使用 [reset-telemetry](https://github.com/wisdgod/cursor-api/tree/main/tools/reset-telemetry) 重置当前用户遥测数据，仅支持windows、linux与macos

## 鸣谢

感谢以下项目和贡献者:

- [cursor-api](https://github.com/wisdgod/cursor-api) - 本项目本身
- [zhx47/cursor-api](https://github.com/zhx47/cursor-api) - 提供了本项目起步阶段的主要参考
- [luolazyandlazy/cursorToApi](https://github.com/luolazyandlazy/cursorToApi)

### 偷偷写在最后的话

虽然作者觉得~骗~收点钱合理，但不强求，要是**主动自愿**发我我肯定收（因为真有人这么做，虽然不是赞助），赞助很合理吧

不是**主动自愿**就算了，不是很缺，给了会很感动罢了。

虽然不是很建议你赞助，但如果你赞助了，大概可以：

* 测试版更新
* 要求功能
* 问题更快解决

即使如此，我也保留可以拒绝赞助和拒绝要求的权利。

求赞助还是有点不要脸了，接下来是吐槽：

辛辛苦苦做这个也不知道是为了谁，好累。其实还有很多功能可以做，比如直接传token支持配置（其实这个要专门做一个页面），这个作为rc.4的计划之一吧。

主要没想做用户管理，所以不存在是否接入LinuxDo的问题。虽然那个半成品公益版做好了就是了。

就说这么多，没啥可说的，不管那么多，做就完了。\[doge\] 自己想象吧。

为什么一直说要跑路呢？主要是有时Cursor的Claude太假了，堪比gpt-4o-mini，我对比发现真没啥差别，比以前差远了，无力了，所以不太想做了。我也感觉很奇怪。

查询额度会在一开始检测导致和完成时的额度有些差别，但是懒得改了，反正差别不大，对话也没响应内容，恰好完成了统一。

有人说少个二维码来着，还是算了。如果觉得好用，给点支持。其实没啥大不了的，没兴趣就不做了。不想那么多了。

要不给我邮箱发口令红包？休息休息