//! Rig 框架适配器模块
//!
//! 本模块提供将现有工具系统适配到 Rig 框架的功能，支持：
//! - 基本工具集成
//! - 动态工具检索（基于向量相似度）
//!
//! 本模块使用泛型设计，支持任何 Rig 支持的 LLM 提供商。

use rig::client::{CompletionClient, EmbeddingsClient, ProviderClient};
use rig::tool::ToolSet;
use rig::vector_store::in_memory_store::InMemoryVectorStore;

use crate::llm::tool::get_tools;


/// 创建带有静态工具的 Rig Agent（通用版本）
///
/// 所有工具都会在每次请求时发送给 LLM。
/// 适合工具数量较少（< 10）的场景。
///
/// # 参数
/// * `client` - 任何实现了 CompletionClient 的 Rig 客户端
/// * `model` - 模型名称
/// * `preamble` - 系统提示
/// * `tools` - 工具列表
///
/// # 返回
/// 配置好的 Agent
pub async fn create_rig_agent<C>(
    client: C,
    model: &str,
    preamble: &str,
) -> Result<rig::agent::Agent<C::CompletionModel>, anyhow::Error>
where
    C: ProviderClient + CompletionClient + Clone,
{
    let agent = client
        .agent(model)
        .preamble(preamble)
        .tools(get_tools())
        .build();

    Ok(agent)
}

/// 创建带有动态工具检索的 Rig Agent（通用版本）
///
/// 使用向量存储根据用户查询自动检索最相关的工具。
/// 适合工具数量较多（>= 10）的场景，可以减少 token 消耗并提高工具选择准确性。
///
/// # 参数
/// * `client` - 任何实现了 CompletionClient 和 EmbeddingsClient 的 Rig 客户端
/// * `model` - 模型名称（用于对话）
/// * `embedding_model` - 嵌入模型名称（用于向量检索）
/// * `preamble` - 系统提示
/// * `dynamic_tool_count` - 每次检索的工具数量（建议 3-10）
///
/// # 返回
/// 配置好的 Agent
pub async fn create_rig_agent_with_dynamic_tools<C>(
    client: C,
    model: &str,
    embedding_model: &str,
    preamble: &str,
    dynamic_tool_count: usize,
) -> Result<rig::agent::Agent<C::CompletionModel>, anyhow::Error>
where
    C: ProviderClient + CompletionClient + EmbeddingsClient + Clone,
    C::EmbeddingModel: Clone + 'static,
{
    // 创建 ToolSet
    let toolset = ToolSet::from_tools_boxed(get_tools());
    
    // 获取嵌入模型（使用指定的嵌入模型名称）
    let embedding_model = client.embedding_model(embedding_model);
    
    // 将工具转换为 schemas（用于嵌入）
    let schemas = toolset.schemas()?;
    
    // 创建嵌入构建器
    let mut builder = rig::embeddings::EmbeddingsBuilder::new(embedding_model.clone());
    
    // 添加每个工具的文档到嵌入构建器
    for schema in schemas {
        builder = builder.document(schema)?;
    }
    
    
    // 构建嵌入
    let embeddings = builder.build().await?;
    
    // 创建向量存储
    let vector_store = InMemoryVectorStore::from_documents(embeddings);
    
    // 创建索引
    let index = vector_store.index(embedding_model);
    
    // 创建 Agent 并添加动态工具
    let agent = client
        .agent(model)
        .preamble(preamble)
        .tools(get_tools())
        .dynamic_tools(dynamic_tool_count, index, toolset)
        .build();
    
    Ok(agent)
}