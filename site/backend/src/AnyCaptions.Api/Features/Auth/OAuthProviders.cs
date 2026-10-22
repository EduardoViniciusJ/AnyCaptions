using Microsoft.AspNetCore.Authentication;
using Microsoft.AspNetCore.Authentication.OAuth;

namespace AnyCaptions.Api.Features.Auth;

public static class OAuthProviders
{
    public static void Configure(OAuthOptions options, IConfiguration configuration, string provider, Uri frontendOrigin)
    {
        options.ClientId = configuration[$"Authentication:{provider}:ClientId"] ?? "";
        options.ClientSecret = configuration[$"Authentication:{provider}:ClientSecret"] ?? "";
        options.SignInScheme = Microsoft.AspNetCore.Authentication.Cookies.CookieAuthenticationDefaults.AuthenticationScheme;
        options.CallbackPath = $"/api/auth/callback/{provider.ToLowerInvariant()}";
        options.SaveTokens = false;
        if (provider == "GitHub") options.Scope.Add("user:email");
        if (provider == "Discord") options.Scope.Add("email");
        options.Events.OnCreatingTicket = context => AuthStateService.CompleteAsync(context, provider);
        options.Events.OnRemoteFailure = context =>
        {
            context.Response.Redirect(new Uri(frontendOrigin, "/account?login=failed").ToString());
            context.HandleResponse();
            return Task.CompletedTask;
        };
    }
}
