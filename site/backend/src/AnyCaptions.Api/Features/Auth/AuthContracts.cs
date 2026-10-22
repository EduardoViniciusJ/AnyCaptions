namespace AnyCaptions.Api.Features.Auth;

public sealed record AccountResponse(Guid Id, string? Email, string Name, string? AvatarUrl);
