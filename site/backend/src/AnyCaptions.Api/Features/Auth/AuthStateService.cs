using System.Security.Claims;
using Microsoft.AspNetCore.Authentication.OAuth;

namespace AnyCaptions.Api.Features.Auth;

public static class AuthStateService
{
    public static async Task CompleteAsync(OAuthCreatingTicketContext context, string provider)
    {
        var providerKey = context.Principal?.FindFirstValue(ClaimTypes.NameIdentifier);
        if (string.IsNullOrWhiteSpace(providerKey) || context.Identity is null)
            throw new InvalidOperationException($"{provider} did not return an account identifier.");

        var profile = context.User;
        string? JsonString(string property) => profile.TryGetProperty(property, out var value)
            && value.ValueKind == System.Text.Json.JsonValueKind.String ? value.GetString() : null;
        var email = context.Principal?.FindFirstValue(ClaimTypes.Email) ?? JsonString("email");
        var name = context.Principal?.FindFirstValue(ClaimTypes.Name)
            ?? JsonString("name") ?? JsonString("login") ?? JsonString("username") ?? provider;
        var avatar = provider switch
        {
            "Google" => JsonString("picture"),
            "GitHub" => JsonString("avatar_url"),
            "Discord" => DiscordAvatar(providerKey, JsonString("avatar")),
            _ => null
        };
        var user = await context.HttpContext.RequestServices.GetRequiredService<AuthService>()
            .SignInAsync(provider, providerKey, email, name, avatar, context.HttpContext.RequestAborted);

        foreach (var claim in context.Identity.FindAll(ClaimTypes.NameIdentifier).ToArray())
            context.Identity.RemoveClaim(claim);
        context.Identity.AddClaim(new Claim(ClaimTypes.NameIdentifier, user.Id.ToString()));
    }

    private static string? DiscordAvatar(string id, string? avatar) =>
        string.IsNullOrWhiteSpace(avatar) ? null : $"https://cdn.discordapp.com/avatars/{id}/{avatar}.png";
}
