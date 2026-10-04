use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn main() {
    println!("🔌 Simulateur Legrand Green'Up démarré sur 127.0.0.1:8080");
    println!("👉 Tapez une commande (ex: 'CC:16', 'State:C', 'T2COK') et appuyez sur Entrée.");
    println!("👉 En attente de connexion de greenup-link-standalone...");

    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();

    // Pour l'instant, on n'accepte qu'un seul client à la fois
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                println!("✅ Client connecté !");
                let mut stream_clone = stream.try_clone().unwrap();

                // Thread 1: Écoute ce que le client (standalone) envoie et l'affiche
                thread::spawn(move || {
                    let mut buf = [0; 1024];
                    let mut buffer_str = String::new();
                    loop {
                        match stream_clone.read(&mut buf) {
                            Ok(0) => {
                                println!("❌ Client déconnecté.");
                                break;
                            }
                            Ok(n) => {
                                let text = String::from_utf8_lossy(&buf[..n]);
                                buffer_str.push_str(&text);
                                
                                while let Some(pos) = buffer_str.find('\r') {
                                    let line = buffer_str[..pos].trim().to_string();
                                    buffer_str = buffer_str[pos + 1..].to_string();
                                    if !line.is_empty() {
                                        println!("[Reçu du client] {}", line);
                                    }
                                }
                            }
                            Err(e) => {
                                println!("Erreur de lecture: {}", e);
                                break;
                            }
                        }
                    }
                });

                // Thread Principal: Lit l'entrée standard (clavier) et l'envoie au client
                let stdin = std::io::stdin();
                let mut line = String::new();
                loop {
                    line.clear();
                    if stdin.read_line(&mut line).is_ok() {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            let mut to_send = trimmed.to_string();
                            to_send.push('\r'); // Ajout du terminateur attendu par la borne/standalone
                            
                            if let Err(e) = stream.write_all(to_send.as_bytes()) {
                                println!("Erreur d'envoi: {}", e);
                                break; // Le client a du se déconnecter
                            } else {
                                println!("[Envoyé au client] {}", trimmed);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                println!("Erreur de connexion: {}", e);
            }
        }
    }
}
