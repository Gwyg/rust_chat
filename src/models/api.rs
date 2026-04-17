use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct RegisterResponse {
    pub token: String,
}

/// 创建群组请求体
#[derive(Deserialize)]
pub struct CreateGroupRequest {
    pub name: String,           // 群名称
}

/// 群成员操作请求体（踢人 / 加人）
#[derive(Deserialize)]
pub struct GroupMemberRequest {
    pub group_id: String,       // 目标群 ID
    pub username: String,       // 被操作的用户名
}

/// 更新群公告请求体
#[derive(Deserialize)]
pub struct UpdateGroupNoticeRequest {
    pub group_id: String,       // 目标群 ID
    pub notice: String,         // 新公告内容
}

/// 更新群头像请求体
#[derive(Deserialize)]
pub struct UpdateGroupAvatarRequest {
    pub group_id: String,       // 目标群 ID
    pub avatar: String,         // 头像内容（base64 字符串或 URL）
}

/// 群组信息响应体
#[derive(Serialize)]
pub struct GroupItem {
    pub group_id: String,       // 群唯一标识
    pub name: String,           // 群名称
    pub owner: String,          // 群主
    pub avatar: Option<String>, // 群头像
    pub notice: Option<String>, // 群公告
    pub member_count: i64,      // 成员数量
    pub role: String,           // 当前用户在群内的角色
}

/// 群成员信息响应体
#[derive(Serialize)]
pub struct GroupMemberItem {
    pub username: String,       // 用户名
    pub role: String,           // 角色：owner / admin / member
    pub joined_at: String,      // 加入时间
}