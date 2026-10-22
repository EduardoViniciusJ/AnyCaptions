using System.Security.Claims;
using Microsoft.AspNetCore.Authentication;
using Microsoft.AspNetCore.Authentication.Cookies;

namespace AnyCaptions.Api.Features.Auth;

public static class AuthEndpoints
{
    public static void MapAuthEndpoints(this WebApplication app, Uri frontendOrigin)
    {
        var group = app.MapGroup("/api/auth");
        group.MapGet("/login/{provider}", (string provider, IConfiguration configuration) =>
        {
            var scheme = provider.ToLowerInvariant() switch
            {
                "google" => "Google",
                "github" => "GitHub",
                "discord" => "Discord",
                _ => null
            };
            if (scheme is null) return Results.NotFound();
            if (string.IsNullOrWhiteSpace(configuration[$"Authentication:{scheme}:ClientId"])
                || string.IsNullOrWhiteSpace(configuration[$"Authentication:{scheme}:ClientSecret"]))
                return Results.StatusCode(StatusCodes.Status503ServiceUnavailable);
            return Results.Challenge(new AuthenticationProperties
                    { RedirectUri = new Uri(frontendOrigin, "/account").ToString() }, [scheme]);
        });
        // The OAuth middleware owns /callback/{provider}, validates state/correlation,
        // exchanges the code and issues the application cookie.
        group.MapGet("/me", async (HttpContext context, ClaimsPrincipal principal, AuthService auth, CancellationToken ct) =>
        {
            context.Response.Headers.CacheControl = "no-store";
            var rawId = principal.FindFirstValue(ClaimTypes.NameIdentifier);
            if (!Guid.TryParse(rawId, out var id)) return Results.Unauthorized();
            var user = await auth.FindUserAsync(id, ct);
            return user is null ? Results.Unauthorized() : Results.Ok(
                new AccountResponse(user.Id, user.Email, user.Name, user.AvatarUrl));
        }).RequireAuthorization();
        group.MapPost("/logout", async (HttpContext context) =>
        {
            if (context.Request.Headers.Origin != frontendOrigin.GetLeftPart(UriPartial.Authority))
                return Results.StatusCode(StatusCodes.Status403Forbidden);
            await context.SignOutAsync(CookieAuthenticationDefaults.AuthenticationScheme);
            return Results.NoContent();
        });
    }
}
