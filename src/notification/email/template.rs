pub const MAIL_CONFIRMATION_TEMPLATE: &str = r#"
<!doctype html>
<html lang="en">
    <head>
        <meta http-equiv="Content-Type" content="text/html; charset=UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
        <title>Confirm your E-Mail</title>
    </head>
    <body style="margin:0; padding:0; background:#ffffff;">
        <table role="presentation" cellpadding="0" cellspacing="0" border="0" width="100%">
            <tr>
                <td align="center">
                    <table role="presentation" cellpadding="0" cellspacing="0" border="0" width="650" class="container" style="width:650px; max-width:650px;">
                        <tr>
                            <td class="px" style="padding:18px 24px; background:#fefbf1;">
                                <img src="{logo_link}"
                                     alt="Bitcredit" width="120" height="24"
                                     style="display:block; border:0; outline:none; text-decoration:none; height:auto;">
                            </td>
                        </tr>
                    </table>
                    <table role="presentation" cellpadding="0" cellspacing="0" border="0" width="650" class="container" style="width:650px; max-width:650px; background:#ffffff;">
                        <tr style="background: #fefbf1;">
                            <td class="px" style="padding:15px 24px 8px 24px; font-family:Geist, system-ui, sans-serif; color:#111111;">
                                <h1 style="margin:0; font-size:24px; line-height:36px; font-weight:500;">
                                    Please confirm your E-Mail
                                </h1>
                            </td>
                        </tr>
                        <tr>
                            <td align="center" style="padding:60px 24px 30px 24px;">
                                <a href="{link}"
                                   style="background:#2b2118; color:#ffffff; text-decoration:none; display:inline-block;
                                          font-family:Geist, system-ui, sans-serif; font-size:14px; font-weight: 500;
                                          padding:12px 24px; border-radius:.5rem;">
                                    Click here to confirm
                                </a>
                            </td>
                        </tr>
                        <tr>
                            <td align="center" class="px" style="padding:0px 24px 28px 24px; font-family:Geist, system-ui, sans-serif; font-size:13px; line-height:20px; color:#333333;">
                                The link is valid for 1 day.
                            </td>
                        </tr>
                    </table>
                    <table role="presentation" cellpadding="0" cellspacing="0" border="0" width="650" class="container" style="width:650px; max-width:650px;">
                        <tr><td style="height:24px; line-height:24px;">&nbsp;</td></tr>
                    </table>
                </td>
            </tr>
        </table>
    </body>
</html>
"#;

#[allow(unused)]
pub const NOTIFICATION_MAIL_TEMPLATE: &str = r#"
<!doctype html>
<html lang="en">
    <head>
        <meta http-equiv="Content-Type" content="text/html; charset=UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
        <title>{title}</title>
    </head>
    <body style="margin:0; padding:0; background:#ffffff;">
        <table role="presentation" cellpadding="0" cellspacing="0" border="0" width="100%">
            <tr>
                <td align="center">
                    <table role="presentation" cellpadding="0" cellspacing="0" border="0" width="650" class="container" style="width:650px; max-width:650px;">
                        <tr>
                            <td class="px" style="padding:18px 24px; background:#fefbf1;">
                                <img src="{logo_link}"
                                     alt="Bitcredit" width="120" height="24"
                                     style="display:block; border:0; outline:none; text-decoration:none; height:auto;">
                            </td>
                        </tr>
                    </table>
                    <table role="presentation" cellpadding="0" cellspacing="0" border="0" width="650" class="container" style="width:650px; max-width:650px; background:#ffffff;">
                        <tr style="background: #fefbf1;">
                            <td class="px" style="padding:15px 24px 8px 24px; font-family:Geist, system-ui, sans-serif; color:#111111;">
                                <h1 style="margin:0; font-size:24px; line-height:36px; font-weight:500;">
                                    {title}
                                </h1>
                            </td>
                        </tr>
                        <tr>
                            <td align="center" style="padding:60px 24px 36px 24px;">
                                <a href="{link}"
                                   style="background:#2b2118; color:#ffffff; text-decoration:none; display:inline-block;
                                          font-family:Geist, system-ui, sans-serif; font-size:14px; font-weight: 500;
                                          padding:12px 24px; border-radius:.5rem;">
                                    Go to eBill
                                </a>
                            </td>
                        </tr>
                        <table role="presentation" cellpadding="0" cellspacing="0" border="0" width="650" class="container" style="width:650px; max-width:650px;">
                            <tr><td style="height:44px; line-height:44px;">&nbsp;</td></tr>
                        </table>
                        <tr>
                            <td align="center" class="px" style="padding:16px 24px 28px 24px; font-family:Geist, system-ui, sans-serif; font-size:13px; line-height:20px; color:#333333;">
                                <a href="{notification_link}" style="color:#333333; text-decoration:none;">Manage notification settings</a>
                                &nbsp;&nbsp;&nbsp;&nbsp;
                                <a href="{browser_link}" style="color:#333333; text-decoration:none;">View in the browser</a>
                            </td>
                        </tr>
                    </table>
                    <table role="presentation" cellpadding="0" cellspacing="0" border="0" width="650" class="container" style="width:650px; max-width:650px;">
                        <tr><td style="height:24px; line-height:24px;">&nbsp;</td></tr>
                    </table>
                </td>
            </tr>
        </table>
    </body>
</html>
"#;
