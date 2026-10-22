using AnyCaptions.Api.Data;
using AnyCaptions.Api.Features.Auth;
using AnyCaptions.Api.Releases;
using Microsoft.AspNetCore.Authentication.Cookies;
using Microsoft.AspNetCore.DataProtection;
using Microsoft.AspNetCore.HttpOverrides;
using Microsoft.EntityFrameworkCore;

var builder = WebApplication.CreateBuilder(args);
var frontendUrl = builder.Configuration["FrontendUrl"]?.TrimEnd('/')
    ?? throw new InvalidOperationException("FrontendUrl is required.");
if (!Uri.TryCreate(frontendUrl, UriKind.Absolute, out var frontendOrigin)
    || (frontendOrigin.Scheme != Uri.UriSchemeHttps && frontendOrigin.Host != "localhost")
    || frontendOrigin.AbsolutePath != "/" || !string.IsNullOrEmpty(frontendOrigin.Query)
    || !string.IsNullOrEmpty(frontendOrigin.Fragment))
    throw new InvalidOperationException("FrontendUrl must be an HTTPS origin (localhost may use HTTP).");

builder.Services.Configure<ForwardedHeadersOptions>(options =>
{
    options.ForwardedHeaders = ForwardedHeaders.XForwardedFor | ForwardedHeaders.XForwardedProto;
    var trustedNetwork = builder.Configuration["TrustedProxyNetwork"];
    if (!string.IsNullOrWhiteSpace(trustedNetwork))
        options.KnownIPNetworks.Add(System.Net.IPNetwork.Parse(trustedNetwork));
});
builder.Services.AddDbContext<AppDbContext>(options =>
    options.UseNpgsql(DatabaseConnection.Resolve(builder.Configuration)));
builder.Services.AddDataProtection().PersistKeysToDbContext<AppDbContext>()
    .SetApplicationName("AnyCaptions.Site");
builder.Services.AddScoped<AuthService>();
builder.Services.AddAuthentication(CookieAuthenticationDefaults.AuthenticationScheme)
    .AddCookie(options =>
    {
        options.Cookie.Name = "__Host-AnyCaptions.Session";
        options.Cookie.HttpOnly = true;
        options.Cookie.SecurePolicy = CookieSecurePolicy.Always;
        options.Cookie.SameSite = SameSiteMode.Lax;
        options.Cookie.Path = "/";
        options.ExpireTimeSpan = TimeSpan.FromDays(14);
        options.SlidingExpiration = true;
        options.Events.OnRedirectToLogin = context =>
        {
            context.Response.StatusCode = StatusCodes.Status401Unauthorized;
            return Task.CompletedTask;
        };
    })
    .AddGoogle("Google", options => OAuthProviders.Configure(options, builder.Configuration, "Google", frontendOrigin))
    .AddGitHub("GitHub", options => OAuthProviders.Configure(options, builder.Configuration, "GitHub", frontendOrigin))
    .AddDiscord("Discord", options => OAuthProviders.Configure(options, builder.Configuration, "Discord", frontendOrigin));
builder.Services.AddAuthorization();
builder.Services.AddCors(options => options.AddPolicy("Frontend", policy =>
    policy.WithOrigins(frontendUrl).AllowAnyHeader().WithMethods("GET", "POST").AllowCredentials()));

var app = builder.Build();
app.UseForwardedHeaders();
if (!app.Environment.IsDevelopment()) app.UseHsts();
app.UseCors("Frontend");
app.UseAuthentication();
app.UseAuthorization();
app.MapGet("/health", () => Results.Ok(new { status = "ok" }));
app.MapAuthEndpoints(frontendOrigin);
app.MapReleaseEndpoints();

await using (var scope = app.Services.CreateAsyncScope())
    await scope.ServiceProvider.GetRequiredService<AppDbContext>().Database.MigrateAsync();

app.Run();

public partial class Program;
