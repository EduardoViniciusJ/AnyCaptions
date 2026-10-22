using AnyCaptions.Api.Data;
using AnyCaptions.Api.Domain.Entities;
using Microsoft.EntityFrameworkCore;

namespace AnyCaptions.Api.Features.Auth;

public sealed class AuthService(AppDbContext db)
{
    public async Task<User> SignInAsync(
        string provider, string providerKey, string? email, string name, string? avatarUrl,
        CancellationToken cancellationToken)
    {
        var login = await db.UserLogins.Include(item => item.User)
            .SingleOrDefaultAsync(item => item.Provider == provider && item.ProviderKey == providerKey,
                cancellationToken);
        var user = login?.User;
        if (user is null)
        {
            user = new User { Name = name, Email = email, AvatarUrl = avatarUrl };
            db.Users.Add(user);
            db.UserLogins.Add(new UserLogin
            {
                User = user,
                Provider = provider,
                ProviderKey = providerKey
            });
        }
        else
        {
            user.Name = name;
            user.Email = email;
            user.AvatarUrl = avatarUrl;
            user.LastLoginAt = DateTimeOffset.UtcNow;
        }
        await db.SaveChangesAsync(cancellationToken);
        return user;
    }

    public Task<User?> FindUserAsync(Guid id, CancellationToken cancellationToken) =>
        db.Users.AsNoTracking().SingleOrDefaultAsync(user => user.Id == id, cancellationToken);
}
