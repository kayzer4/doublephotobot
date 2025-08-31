use teloxide::{net::Download, prelude::*, types::{MediaKind, MessageKind}};
use std::error::Error;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use image::imageops::FilterType;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().expect("Failed to load .env file");
    pretty_env_logger::init();
    log::info!("Starting debug bot...");

    let bot = Bot::from_env();

    teloxide::repl(bot, |bot: Bot, msg: Message| async move {
        if let MessageKind::Common(common) = &msg.kind {
            if let MediaKind::Photo(media_photo) = &common.media_kind {
                let caption = &media_photo.caption;
                
                if caption.is_none() || caption.as_ref().unwrap().is_empty() {
                    bot.send_message(msg.chat.id, "❌ Отсутствует название фото! Добавьте название к описанию фото.").await?;
                    return Ok(());
                }
                
                if let Some(largest_photo) = media_photo.photo.last() {
                    let file_id = largest_photo.file.id.clone();
                    match bot.get_file(file_id).await {
                        Ok(file) => {
                            let user_id = msg.from.as_ref().unwrap().id.0;
                            let user_dir = format!("photos/src/{}", user_id);
                            let out_dir = format!("photos/out/{}", user_id);
                            
                            let _ = tokio::fs::create_dir_all(&user_dir).await;
                            let _ = tokio::fs::create_dir_all(&out_dir).await;
                            
                            let caption_text = caption.as_ref().unwrap();
                            let filename = format!("{}/{}.jpg", user_dir, caption_text);
                            let out_filename = format!("{}/{}.jpg", out_dir, caption_text);
                            
                            match File::create(&filename).await {
                                Ok(mut file_handle) => {
                                    match bot.download_file(&file.path, &mut file_handle).await {
                                        Ok(()) => {
                                            if let Err(e) = file_handle.shutdown().await {
                                                log::error!("Error closing file: {}", e);
                                            }
                                            
                                            let filename_clone = filename.clone();
                                            let out_filename_clone = out_filename.clone();
                                            
                                            match tokio::task::spawn_blocking(move || {
                                                process_image_for_output(&filename_clone, &out_filename_clone)
                                            }).await {
                                                Ok(result) => {
                                                    match result {
                                                        Ok(_) => {
                                                            let photo_input = teloxide::types::InputFile::file(Path::new(&out_filename));
                                                            
                                                            bot.send_document(
                                                                msg.chat.id,
                                                                photo_input
                                                            )
                                                            .caption(format!("✅ Фото сохранено и оптимизировано!\nНазвание: {}", caption_text))
                                                            .await?;
                                                        }
                                                        Err(e) => {
                                                            log::error!("Error processing image: {}", e);
                                                            bot.send_message(
                                                                msg.chat.id, 
                                                                format!("✅ Фото сохранено, но возникла ошибка при оптимизации\nНазвание: {}", caption_text)
                                                            ).await?;
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    log::error!("Error in spawn_blocking: {}", e);
                                                    bot.send_message(
                                                        msg.chat.id, 
                                                        format!("✅ Фото сохранено, но возникла ошибка при обработке\nНазвание: {}", caption_text)
                                                    ).await?;
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            log::error!("Error downloading photo: {}", e);
                                            bot.send_message(msg.chat.id, "❌ Ошибка при загрузке фото").await?;
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("Error creating file: {}", e);
                                    bot.send_message(msg.chat.id, "❌ Ошибка при создании файла").await?;
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Error getting file info: {}", e);
                            bot.send_message(msg.chat.id, "❌ Ошибка при получении информации о файле").await?;
                        }
                    }
                }
            } else {
                bot.send_message(msg.chat.id, "Отправьте фотографию для оптимизации под следующие параметры\n\nРазмер: до 20 КБ\nФормат изображения: JPEG\nРазрешение: 224х298").await?;
            }
        }
        
        Ok(())
    })
    .await;

    Ok(())
}

fn process_image_for_output(input_path: &str, output_path: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let img = image::open(input_path)?;
    
    let resized = img.resize_exact(224, 298, FilterType::Lanczos3);
    
    for _quality in (50..=95).rev().step_by(5) {
        let mut buffer = Vec::new();
        
        resized.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageFormat::Jpeg)?;
        
        if buffer.len() <= 20 * 1024 {
            std::fs::write(output_path, &buffer)?;
            return Ok(());
        }
    }
        
    let mut buffer = Vec::new();
    resized.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageFormat::Jpeg)?;
    std::fs::write(output_path, &buffer)?;
    
    log::warn!("Image size after compression: {} bytes (target: 20KB)", buffer.len());
    
    Ok(())
}