pub const TEMPLATE: &str = r#"
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
  <style>
    :root \{
      --bg: #fefbf1;
      --card: #ffffff;
      --header: #faf5e8;
      --text: #111111;
      --muted: #333333;
      --divider: #e8e6e1;
      --primary: #2b2118;
    }

    * \{
        box-sizing: border-box
    }

    body \{
        margin: 0;
        background: var(--bg);
        color: var(--text);
        font: 16px/1.5 system-ui,Geist, sans-serif;
    }

    .container \{
        max-width: 650px;
        margin: 0 auto;
    }

    .header \{
        background: var(--header);
        padding: 18px 24px;
    }

    .logo \{ 
        display: block;
        height: 24px;
        width: auto;
    }

    .card \{
        background: var(--card);
    }

    .section \{
        padding: 12px 24px;
    }

    h1 \{
        margin: 0;
        font-size: 28px;
        line-height: 1.3;
        font-weight: 700
    }

    .cta-wrap \{
        display: flex;
        justify-content: center;
        padding: 28px 24px 36px;
    }

    .btn \{
        display: inline-block;
        background: var(--primary);
        color: #fff;
        text-decoration: none;
        padding: 12px 24px;
        border-radius: 10px;
        font-weight: 700;
    }

    .divider \{
        height: 1px;
        background: var(--divider);
        margin: 0 24px;
    }
  </style>
</head>
<body>
  <div class="container">
    <div class="header">
      <img class="logo" src="{logo_link}" alt="Bitcredit">
    </div>

    <div class="card">
      <div class="section">
        <h1>{title}</h1>
      </div>

      <div class="section">
          <div class="content">
            {{call content with content}}
          </div>
      </div>

      <div class="divider"></div>
    </div>

    <div style="height:24px"></div>
  </div>
</body>
</html>
"#;

pub const ERROR_SUCCESS_TEMPLATE: &str = r#"
    {msg}
"#;

pub const PREFERENCES_TEMPLATE: &str = r#"
    <h3>for {anon_email} / {anon_npub}</h3>
    <form action="/notifications/update_preferences" method="POST">
        <input type="hidden" name="preferences_token" value="{ preferences_token }"/>
        <div>
            <input {{if enabled}} checked {{endif}} type="checkbox" name="enabled" id="enabled" />
            <label for="enabled">Enabled</label>
        </div>
        <hr />
        {{ for flag in flags }}
        <div>
            <input {{if flag.checked }} checked {{endif}} type="checkbox" name="flags" value="{ flag.value }" id="flag{ flag.value }"/>
            <label for="flag{ flag.value }">{ flag.name }</label>
        </div>
        {{ endfor }}
        <div>
          <div class="cta-wrap">
            <button class="btn" type="submit">Submit</button>
          </div>
        </div>
    </form>
"#;
