const HEADER: &'static str  = r#"<smil>
<head>
<layout>\n"#;
//"r1", "meet"
//"r2", "scroll"
//format!(r#"<region id="{}" height="100%" width="100%" fit="{}"/>\n"#, id, fit)
const MID: &'static str = r#"</layout>
</head>
<body>
<par dur="5s">\n"#;
//"text", "foo.txt", "r1"
//"img", "image.jpg", "r2"
//"video", "vid.mp4", "r3"
//"audio", "song.mp3", "r4"
//"ref", "foo.zip", "r4"
//format!(r#"<{} src="{}" region="{}"/>\n"#, kind, filename, id)
const FOOTER: &'static str = r#"</par>
</body>
</smil>"#;

fn mime_to_fit(mime: &str, ) -> &'static str {
	if mime.starts_with("text/") {
		"scroll"
	} else if mime.starts_with("image/") {
		"meet"
	} else {
		"meet"
	}
}

fn mime_to_tag(mime: &str, ) -> &'static str {
	if mime.starts_with("text/") {
		"text"
	} else if mime.starts_with("image/") {
		"img"
	} else if mime.starts_with("audio/") {
		"audio"
	} else if mime.starts_with("video/") {
		"video"
	} else {
		"ref"
	}
}

pub fn generate_smil(attachments: &[(&str, &str, &str)]) -> String {
	let mut out = String::new();
	out.push_str(HEADER);
	
	let mut rest = String::new();
	out.push_str(MID);
	for (id, att) in attachments.iter().enumerate() {
		let fit = mime_to_fit(att.1);
		out.push_str(&format!(
			r#"<region id="cid-{}" height="100%" width="100%" fit="{}"/>\n"#,
			id, fit));
		let tag = mime_to_tag(att.1);
		rest.push_str(&format!(
			r#"<{} src="{}" region="cid-{}"/>\n"#,
			tag, att.0, id));
	}
	out.push_str(&rest);
	out.push_str(FOOTER);
	out
}
