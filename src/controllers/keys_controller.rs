pub async fn get_keys_for_user(State(pool): State<PgPool>) -> Json<Vec<User>> {
    let users = user_repository::get_all_users(&pool).await.unwrap();
    Json(users)
}